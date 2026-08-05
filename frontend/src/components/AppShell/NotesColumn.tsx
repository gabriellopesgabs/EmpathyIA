'use client';

import { useEffect, useMemo, useState } from 'react';
import { Archive, ArchiveRestore, CircleDot, MoreHorizontal, Pencil, Plus, Search, Trash2 } from 'lucide-react';
import { usePathname, useRouter } from 'next/navigation';
import { toast } from 'sonner';
import { useSidebar, type CurrentMeeting } from '@/components/Sidebar/SidebarProvider';
import { noteService } from '@/services/noteService';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger } from '@/components/ui/dropdown-menu';
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog';

export function NotesColumn() {
  const pathname = usePathname();
  const router = useRouter();
  const { meetings, currentMeeting, setCurrentMeeting, refetchMeetings } = useSidebar();
  const [query, setQuery] = useState('');
  const [showArchived, setShowArchived] = useState(false);
  const [pendingDelete, setPendingDelete] = useState<CurrentMeeting | null>(null);
  const hidden = pathname.startsWith('/settings') || pathname.startsWith('/knowledge');

  useEffect(() => {
    const reveal = () => setShowArchived(true);
    const showActive = () => setShowArchived(false);
    window.addEventListener('show-archived-notes', reveal);
    window.addEventListener('show-active-notes', showActive);
    return () => {
      window.removeEventListener('show-archived-notes', reveal);
      window.removeEventListener('show-active-notes', showActive);
    };
  }, []);

  const visible = useMemo(() => meetings.filter(note => {
    if (Boolean(note.archived) !== showArchived) return false;
    return !query.trim() || note.title.toLocaleLowerCase('pt-BR').includes(query.trim().toLocaleLowerCase('pt-BR'));
  }), [meetings, query, showArchived]);

  if (hidden) return null;

  const open = (note: CurrentMeeting) => {
    setCurrentMeeting(note);
    router.push(note.recorded ? `/meeting-details?id=${note.id}` : `/notes?id=${note.id}`);
  };

  const create = async () => {
    try {
      const note = await noteService.create();
      await refetchMeetings();
      router.push(`/notes?id=${note.id}`);
    } catch (error) {
      toast.error('Não foi possível criar a nota', { description: String(error) });
    }
  };

  const archive = async (note: CurrentMeeting, archived: boolean) => {
    try {
      await noteService.setArchived(note.id, archived);
      await refetchMeetings();
      toast.success(archived ? 'Nota arquivada' : 'Nota restaurada', {
        action: {
          label: 'Desfazer',
          onClick: async () => {
            await noteService.setArchived(note.id, !archived);
            await refetchMeetings();
          },
        },
      });
    } catch (error) {
      toast.error('Não foi possível atualizar a nota', { description: String(error) });
    }
  };

  const remove = async () => {
    if (!pendingDelete) return;
    try {
      await noteService.remove(pendingDelete.id);
      await refetchMeetings();
      setPendingDelete(null);
      if (currentMeeting?.id === pendingDelete.id) router.push('/');
      toast.success('Nota movida para a lixeira recuperável');
    } catch (error) {
      toast.error('Não foi possível excluir a nota', { description: String(error) });
    }
  };

  return (
    <aside className="notes-column" aria-label={showArchived ? 'Notas arquivadas' : 'Lista de notas'}>
      <header className="notes-column-header">
        <div className="flex items-center justify-between gap-2">
          <div>
            <p className="eyebrow">Biblioteca</p>
            <h1>{showArchived ? 'Arquivadas' : 'Notas'}</h1>
          </div>
          <Button variant="ghost" size="icon" onClick={() => void create()} aria-label="Criar nova nota"><Plus /></Button>
        </div>
        <div className="notes-search">
          <Search />
          <Input value={query} onChange={event => setQuery(event.target.value)} placeholder="Buscar" aria-label="Buscar notas" />
        </div>
        {showArchived && <button className="text-link" onClick={() => setShowArchived(false)}>Voltar às notas</button>}
      </header>

      <div className="notes-list">
        {visible.length === 0 && <div className="empty-list"><p>{query ? 'Nenhuma nota encontrada.' : showArchived ? 'Nenhuma nota arquivada.' : 'Sua biblioteca está vazia.'}</p>{!query && !showArchived && <Button variant="outline" size="sm" onClick={() => void create()}>Criar nota</Button>}</div>}
        {visible.map(note => (
          <div key={note.id} className="note-row" data-active={currentMeeting?.id === note.id}>
            <button type="button" className="note-row-main" onClick={() => open(note)}>
              <span className="note-origin" aria-hidden="true">{note.recorded && <CircleDot />}{note.written && <Pencil />}</span>
              <span className="min-w-0"><strong>{note.title}</strong><small>{note.recorded && note.written ? 'Gravada e editada' : note.recorded ? 'Nota gravada' : 'Nota Markdown'}</small></span>
            </button>
            <DropdownMenu>
              <DropdownMenuTrigger asChild><button className="note-menu" aria-label={`Ações para ${note.title}`}><MoreHorizontal /></button></DropdownMenuTrigger>
              <DropdownMenuContent align="end">
                <DropdownMenuItem onSelect={() => void archive(note, !note.archived)}>{note.archived ? <ArchiveRestore /> : <Archive />}{note.archived ? 'Restaurar' : 'Arquivar'}</DropdownMenuItem>
                <DropdownMenuSeparator />
                <DropdownMenuItem className="text-destructive focus:text-destructive" onSelect={() => setPendingDelete(note)}><Trash2 />Excluir…</DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
        ))}
      </div>

      <Dialog open={Boolean(pendingDelete)} onOpenChange={open => !open && setPendingDelete(null)}>
        <DialogContent>
          <DialogHeader><DialogTitle>Excluir “{pendingDelete?.title}”?</DialogTitle><DialogDescription>A nota será retirada da biblioteca e sua pasta será movida para a lixeira recuperável do Empathy.</DialogDescription></DialogHeader>
          <DialogFooter><Button variant="ghost" onClick={() => setPendingDelete(null)}>Cancelar</Button>{!pendingDelete?.archived && <Button variant="outline" onClick={() => pendingDelete && void archive(pendingDelete, true).then(() => setPendingDelete(null))}>Arquivar</Button>}<Button variant="destructive" onClick={() => void remove()}>Excluir</Button></DialogFooter>
        </DialogContent>
      </Dialog>
    </aside>
  );
}
