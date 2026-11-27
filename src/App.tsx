import { useEffect, useState } from "react";
import HostKeyDialog from "./components/HostKeyDialog";
import Sidebar, { type Page } from "./components/Sidebar";
import Toaster from "./components/Toaster";
import TunnelEditor from "./components/TunnelEditor";
import UpdateDialog from "./components/UpdateDialog";
import { events, toolsApi } from "./lib/api";
import type { TunnelView } from "./lib/types";
import Connections from "./pages/Connections";
import Home from "./pages/Home";
import Logs from "./pages/Logs";
import Settings from "./pages/Settings";
import Tools from "./pages/Tools";
import Tunnels from "./pages/Tunnels";

export default function App() {
  const [page, setPage] = useState<Page>("home");
  const [editor, setEditor] = useState<{ open: boolean; tunnel: TunnelView | null }>({ open: false, tunnel: null });
  const edit = (tunnel: TunnelView | null) => setEditor({ open: true, tunnel });

  // 按下快捷键时切到工具页：窗口新建时在加载后取请求，已打开时收到事件再取。
  useEffect(() => {
    const take = () =>
      toolsApi
        .takeOpenRequest()
        .then((yes) => yes && setPage("tools"))
        .catch(() => {});
    take();
    return events.onToolsOpen(take);
  }, []);

  return (
    <div className="flex h-full" onContextMenu={(e) => import.meta.env.PROD && e.preventDefault()}>
      <Sidebar page={page} onChange={setPage} />
      <main className="flex min-w-0 flex-1 flex-col">
        {page === "home" && <Home onNew={() => edit(null)} onNavigate={setPage} />}
        {page === "tunnels" && <Tunnels onEdit={edit} />}
        {page === "connections" && <Connections />}
        {page === "logs" && <Logs />}
        {page === "tools" && <Tools />}
        {page === "settings" && <Settings />}
      </main>
      <TunnelEditor open={editor.open} tunnel={editor.tunnel} onClose={() => setEditor((s) => ({ ...s, open: false }))} />
      <HostKeyDialog />
      <UpdateDialog />
      <Toaster />
    </div>
  );
}
