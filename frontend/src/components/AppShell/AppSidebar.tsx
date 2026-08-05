'use client';

import { Archive, Home, Import, Mic, Network, PanelLeftClose, PanelLeftOpen, Plus, Settings } from 'lucide-react';
import { usePathname, useRouter } from 'next/navigation';
import { useEffect } from 'react';
import { useSidebar } from '@/components/Sidebar/SidebarProvider';
import { useRecordingState } from '@/contexts/RecordingStateContext';
import { useImportDialog } from '@/contexts/ImportDialogContext';
import { noteService } from '@/services/noteService';
import { toast } from 'sonner';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';

const itemClass = 'app-sidebar-item';

export function AppSidebar() {
  const pathname = usePathname();
  const router = useRouter();
  const { isCollapsed, toggleCollapse, handleRecordingToggle } = useSidebar();
  const { isRecording } = useRecordingState();
  const { openImportDialog } = useImportDialog();

  useEffect(() => {
    const openImport = () => openImportDialog();
    const toggle = () => toggleCollapse();
    window.addEventListener('open-import-dialog', openImport);
    window.addEventListener('toggle-app-sidebar', toggle);
    return () => {
      window.removeEventListener('open-import-dialog', openImport);
      window.removeEventListener('toggle-app-sidebar', toggle);
    };
  }, [openImportDialog, toggleCollapse]);

  const createNote = async () => {
    try {
      const note = await noteService.create();
      window.dispatchEvent(new CustomEvent('notes-changed'));
      router.push(`/notes?id=${note.id}`);
    } catch (error) {
      toast.error('Não foi possível criar a nota', { description: String(error) });
    }
  };

  const navItem = (label: string, href: string, Icon: typeof Home) => (
    <Tooltip>
      <TooltipTrigger asChild>
        <button type="button" className={itemClass} data-active={pathname === href} onClick={() => {
          if (href === '/') window.dispatchEvent(new CustomEvent('show-active-notes'));
          router.push(href);
        }}>
          <Icon /><span>{label}</span>
        </button>
      </TooltipTrigger>
      {isCollapsed && <TooltipContent side="right">{label}</TooltipContent>}
    </Tooltip>
  );

  return (
    <aside className="app-sidebar" data-collapsed={isCollapsed} aria-label="Navegação principal">
      <div className="app-drag-region">
        <div className="app-brand" aria-label="Empathy">
          <span className="app-brand-mark">E</span><span>Empathy</span>
        </div>
      </div>

      <nav className="app-sidebar-nav">
        {navItem('Hoje', '/', Home)}
        {navItem('Conhecimento', '/knowledge', Network)}
        <button type="button" className={itemClass} onClick={() => window.dispatchEvent(new CustomEvent('show-archived-notes'))}>
          <Archive /><span>Arquivadas</span>
        </button>
      </nav>

      <div className="app-sidebar-actions">
        <Tooltip>
          <TooltipTrigger asChild>
            <button type="button" className="app-sidebar-primary" onClick={handleRecordingToggle} data-recording={isRecording}>
              <Mic /><span>{isRecording ? 'Gravando…' : 'Gravar'}</span>
            </button>
          </TooltipTrigger>
          {isCollapsed && <TooltipContent side="right">{isRecording ? 'Gravando…' : 'Gravar'}</TooltipContent>}
        </Tooltip>
        <button type="button" className={itemClass} onClick={() => void createNote()}><Plus /><span>Nova nota</span></button>
        <button type="button" className={itemClass} onClick={() => openImportDialog()}><Import /><span>Importar áudio</span></button>
      </div>

      <div className="app-sidebar-footer">
        {navItem('Configurações', '/settings', Settings)}
        <button type="button" className={itemClass} onClick={toggleCollapse} aria-label={isCollapsed ? 'Expandir barra lateral' : 'Recolher barra lateral'}>
          {isCollapsed ? <PanelLeftOpen /> : <PanelLeftClose />}<span>Recolher</span>
        </button>
      </div>
    </aside>
  );
}
