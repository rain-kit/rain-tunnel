//! 业务层测试：保存与密码、复制、删除、指纹确认流程、连接管理。

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::*;
use rain_tunnel::core::ssh::known_hosts::HostKeyStore;
use rain_tunnel::events::{self, EventSink};
use rain_tunnel::model::{AuthType, SaveTunnelReq, Tunnel, TunnelState, TunnelType};
use rain_tunnel::secrets::{MemoryBackend, Secrets};
use rain_tunnel::service::{TunnelService, Tuning};
use rain_tunnel::store::TunnelStore;

#[derive(Default)]
struct RecordSink {
    events: Mutex<Vec<String>>,
    shown: Mutex<usize>,
}

impl EventSink for RecordSink {
    fn emit(&self, event: &str, _payload: serde_json::Value) {
        self.events.lock().unwrap().push(event.to_string());
    }
    fn show_main_window(&self) {
        *self.shown.lock().unwrap() += 1;
    }
}

fn service(sink: Arc<RecordSink>) -> (Arc<TunnelService>, std::path::PathBuf) {
    let dir = temp_dir();
    let tuning = Tuning {
        dial_timeout: Duration::from_secs(5),
        min_backoff: Duration::from_millis(100),
        max_backoff: Duration::from_millis(300),
        prompt_timeout: Duration::from_secs(5),
        ..Default::default()
    };
    let svc = TunnelService::new(
        TunnelStore::new(dir.join("tunnels.json")),
        Secrets::new(Box::new(MemoryBackend::default())),
        Arc::new(HostKeyStore::new(dir.join("known_hosts"), vec![])),
        sink,
        tuning,
    );
    (svc, dir)
}

fn new_tunnel(srv: &TestServer) -> Tunnel {
    Tunnel {
        kind: TunnelType::Dynamic,
        ssh_host: "127.0.0.1".into(),
        ssh_port: srv.addr.port() as i32,
        ssh_user: USER.into(),
        auth_type: AuthType::Password,
        password: PASSWORD.into(),
        listen_host: "127.0.0.1".into(),
        listen_port: 0,
        auto_reconnect: true,
        ..Default::default()
    }
}

async fn wait_state(svc: &TunnelService, id: &str, want: TunnelState) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let v = svc.get(id).unwrap();
        if v.status.state == want {
            return;
        }
        assert!(tokio::time::Instant::now() < deadline, "等待 {want:?} 超时，当前 {:?}：{:?}", v.status.state, v.status.error);
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn save_keeps_secrets_out_of_file() {
    let srv = TestServer::start().await;
    let (svc, dir) = service(Arc::default());

    let v = svc.save(SaveTunnelReq { tunnel: new_tunnel(&srv), ..Default::default() }).await.unwrap();
    assert!(!v.tunnel.id.is_empty());
    assert!(v.has_password);
    assert!(v.tunnel.password.is_empty());
    assert!(v.tunnel.created_at.is_some());
    let json = std::fs::read_to_string(dir.join("tunnels.json")).unwrap();
    assert!(!json.contains(PASSWORD), "密码不应写入文件");
    assert!(json.contains("\"tunnels\""));

    // 密码留空表示保持不变。
    let mut edit = v.tunnel.clone();
    edit.name = "改名".into();
    let v2 = svc.save(SaveTunnelReq { tunnel: edit.clone(), ..Default::default() }).await.unwrap();
    assert_eq!(v2.tunnel.name, "改名");
    assert!(v2.has_password);
    assert_eq!(v2.tunnel.created_at, v.tunnel.created_at);

    // 显式清除。
    let v3 = svc
        .save(SaveTunnelReq { tunnel: edit, clear_password: true, ..Default::default() })
        .await
        .unwrap();
    assert!(!v3.has_password);

    // 校验失败返回参数错误。
    let mut bad = v3.tunnel.clone();
    bad.ssh_host.clear();
    let err = svc.save(SaveTunnelReq { tunnel: bad, ..Default::default() }).await.unwrap_err();
    assert_eq!(err.code, rain_tunnel::error::TUNNEL_INVALID.0);
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn duplicate_and_delete() {
    let srv = TestServer::start().await;
    let (svc, dir) = service(Arc::default());
    let mut t = new_tunnel(&srv);
    t.name = "原始".into();
    t.auto_start = true;
    let v = svc.save(SaveTunnelReq { tunnel: t, ..Default::default() }).await.unwrap();
    let d = svc.duplicate(&v.tunnel.id).await.unwrap();
    assert_ne!(d.tunnel.id, v.tunnel.id);
    assert_eq!(d.tunnel.name, "原始 副本");
    assert!(!d.tunnel.auto_start);
    assert!(d.has_password, "复制时应带上密码");

    let ids: Vec<_> = svc.list().unwrap().iter().map(|t| t.tunnel.id.clone()).collect();
    assert_eq!(ids, vec![v.tunnel.id.clone(), d.tunnel.id.clone()]);
    svc.reorder(&[d.tunnel.id.clone(), v.tunnel.id.clone()]).unwrap();
    assert_eq!(svc.list().unwrap()[0].tunnel.id, d.tunnel.id);

    svc.delete(&v.tunnel.id).await.unwrap();
    assert_eq!(svc.list().unwrap().len(), 1);
    assert_eq!(svc.get(&v.tunnel.id).unwrap_err().code, rain_tunnel::error::TUNNEL_NOT_FOUND.0);
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn host_key_prompt_flow() {
    let srv = TestServer::start().await;
    let sink = Arc::new(RecordSink::default());
    let (svc, dir) = service(sink.clone());
    let v = svc.save(SaveTunnelReq { tunnel: new_tunnel(&srv), ..Default::default() }).await.unwrap();
    let id = v.tunnel.id.clone();

    svc.start(&id).await.unwrap();
    let prompt = loop {
        if let Some(p) = svc.pending_host_keys().pop() {
            break p;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    assert_eq!(prompt.tunnel_id, id);
    assert!(prompt.fingerprint.starts_with("SHA256:"));
    assert_eq!(*sink.shown.lock().unwrap(), 1, "需要确认时应显示主窗口");
    assert!(sink.events.lock().unwrap().iter().any(|e| e == events::HOST_KEY));

    svc.answer_host_key(&prompt.request_id, true).unwrap();
    wait_state(&svc, &id, TunnelState::Connected).await;
    assert!(svc.pending_host_keys().is_empty());
    assert_eq!(
        svc.answer_host_key(&prompt.request_id, true).unwrap_err().code,
        rain_tunnel::error::HOST_KEY_EXPIRED.0
    );
    assert_eq!(svc.running_count(), (1, 1));
    assert!(sink.events.lock().unwrap().iter().any(|e| e == events::TUNNEL_STATUS));

    // 重复启动无副作用；停止后从运行列表移除。
    svc.start(&id).await.unwrap();
    svc.stop(&id).await;
    assert_eq!(svc.running_count(), (0, 0));
    assert_eq!(svc.get(&id).unwrap().status.state, TunnelState::Stopped);
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn edit_restarts_running_tunnel_and_start_all() {
    let srv = TestServer::start().await;
    let (svc, dir) = service(Arc::default());
    let mut a = new_tunnel(&srv);
    a.name = "a".into();
    a.auto_start = true;
    let a = svc.save(SaveTunnelReq { tunnel: a, ..Default::default() }).await.unwrap();
    let mut b = new_tunnel(&srv);
    b.name = "b".into();
    b.password = "wrong".into();
    let b = svc.save(SaveTunnelReq { tunnel: b, ..Default::default() }).await.unwrap();

    svc.start(&a.tunnel.id).await.unwrap();
    let p = loop {
        if let Some(p) = svc.pending_host_keys().pop() {
            break p;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    svc.answer_host_key(&p.request_id, true).unwrap();
    wait_state(&svc, &a.tunnel.id, TunnelState::Connected).await;

    // 修改运行中的隧道会自动重启以应用新配置。
    let mut edit = a.tunnel.clone();
    edit.listen_port = 0;
    edit.enable_http = true;
    svc.save(SaveTunnelReq { tunnel: edit, ..Default::default() }).await.unwrap();
    wait_state(&svc, &a.tunnel.id, TunnelState::Connected).await;
    assert!(svc.get(&a.tunnel.id).unwrap().tunnel.enable_http);

    svc.stop_all().await;
    assert_eq!(svc.running_count(), (0, 0));

    let failed = svc.start_auto().await;
    assert!(failed.is_empty());
    assert_eq!(svc.running_count().0, 1);

    svc.start_all().await;
    wait_state(&svc, &b.tunnel.id, TunnelState::Failed).await;
    // 失败的隧道可以直接再次启动。
    svc.start(&b.tunnel.id).await.unwrap();
    wait_state(&svc, &b.tunnel.id, TunnelState::Failed).await;
    svc.stop_all().await;
    let _ = std::fs::remove_dir_all(dir);
}
