'use client';

import { useCallback, useEffect, useMemo, useState } from 'react';
import { useRouter } from 'next/navigation';
import { invoke } from '@tauri-apps/api/core';
import {
  BookOpen, Boxes, CheckSquare, FileInput, GitBranch, Globe2,
  Loader2, Network, RefreshCw, Search, Sparkles, Users,
} from 'lucide-react';
import { toast } from 'sonner';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Textarea } from '@/components/ui/textarea';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Dialog, DialogContent, DialogTitle } from '@/components/ui/dialog';

type CountedValue = { value: string; count: number };
type KnowledgeTask = {
  id: string; meeting_id?: string; document_path: string; text: string;
  owner?: string; completed: boolean; line_number: number;
};
type KnowledgeDecision = {
  id: string; meeting_id?: string; document_path: string; text: string; line_number: number;
};
type Dashboard = {
  documents: number; meetings: number; projects: CountedValue[];
  participants: CountedValue[]; tags: CountedValue[];
  open_tasks: KnowledgeTask[]; recent_decisions: KnowledgeDecision[];
};
type ReindexResult = {
  root: string; documents: number; meetings: number; links: number;
  tasks: number; decisions: number; errors: string[];
};
type SearchResult = {
  path: string; meeting_id?: string; kind: string; title: string;
  project?: string; snippet: string; score: number;
};
type KnowledgeExtension = {
  id: string; name: string; description: string; action: string;
  enabled: boolean; source_path: string;
};
type KnowledgeDocumentContent = { path: string; title: string; kind: string; content: string };

const emptyDashboard: Dashboard = {
  documents: 0, meetings: 0, projects: [], participants: [], tags: [],
  open_tasks: [], recent_decisions: [],
};

function StatCard({ icon: Icon, label, value }: { icon: typeof BookOpen; label: string; value: number }) {
  return (
    <div className="rounded-xl border bg-white p-5 shadow-sm dark:bg-gray-900">
      <div className="flex items-center gap-3 text-gray-500">
        <Icon className="h-5 w-5" />
        <span className="text-sm">{label}</span>
      </div>
      <p className="mt-3 text-3xl font-semibold text-gray-900 dark:text-white">{value}</p>
    </div>
  );
}

function ValueCloud({ title, values, onSelect }: {
  title: string; values: CountedValue[]; onSelect: (value: string) => void;
}) {
  return (
    <section className="rounded-xl border bg-white p-5 shadow-sm dark:bg-gray-900">
      <h2 className="mb-3 font-semibold">{title}</h2>
      <div className="flex flex-wrap gap-2">
        {values.length === 0 && <span className="text-sm text-gray-500">Ainda não identificado.</span>}
        {values.slice(0, 16).map(({ value, count }) => (
          <button key={value} onClick={() => onSelect(value)}
            className="rounded-full border bg-gray-50 px-3 py-1.5 text-sm hover:bg-gray-100 dark:bg-gray-800">
            {value} <span className="text-gray-400">{count}</span>
          </button>
        ))}
      </div>
    </section>
  );
}

export default function KnowledgePage() {
  const router = useRouter();
  const [dashboard, setDashboard] = useState<Dashboard>(emptyDashboard);
  const [lastIndex, setLastIndex] = useState<ReindexResult | null>(null);
  const [loading, setLoading] = useState(true);
  const [query, setQuery] = useState('');
  const [savedSearches, setSavedSearches] = useState<string[]>([]);
  const [results, setResults] = useState<SearchResult[]>([]);
  const [searching, setSearching] = useState(false);
  const [activeTab, setActiveTab] = useState('overview');
  const [extensions, setExtensions] = useState<KnowledgeExtension[]>([]);
  const [preview, setPreview] = useState<KnowledgeDocumentContent | null>(null);
  const [context, setContext] = useState({ title: '', url: '', project: '', tags: '', content: '' });

  const loadDashboard = useCallback(async () => {
    const data = await invoke<Dashboard>('api_get_knowledge_dashboard');
    setDashboard(data);
  }, []);

  const reindex = useCallback(async (quiet = false) => {
    if (!quiet) setLoading(true);
    try {
      const result = await invoke<ReindexResult>('api_reindex_knowledge');
      setLastIndex(result);
      await loadDashboard();
      if (!quiet) {
        toast.success('Biblioteca Markdown atualizada', {
          description: `${result.documents} documentos, ${result.tasks} tarefas e ${result.decisions} decisões.`,
        });
      }
    } catch (error) {
      toast.error('Não foi possível indexar a biblioteca', { description: String(error) });
    } finally {
      setLoading(false);
    }
  }, [loadDashboard]);

  useEffect(() => {
    try {
      setSavedSearches(JSON.parse(localStorage.getItem('empathy_saved_knowledge_searches') || '[]'));
    } catch {
      setSavedSearches([]);
    }
  }, []);

  useEffect(() => {
    const refresh = () => loadDashboard().finally(() => setLoading(false));
    refresh();
    window.addEventListener('knowledge-index-updated', refresh);
    return () => {
      window.removeEventListener('knowledge-index-updated', refresh);
    };
  }, [loadDashboard]);

  useEffect(() => {
    const timer = setTimeout(async () => {
      if (!query.trim()) {
        setResults([]);
        return;
      }
      setSearching(true);
      try {
        setResults(await invoke<SearchResult[]>('api_search_knowledge', { query }));
      } finally {
        setSearching(false);
      }
    }, 250);
    return () => clearTimeout(timer);
  }, [query]);

  const saveSearch = () => {
    const value = query.trim();
    if (!value || savedSearches.includes(value)) return;
    const next = [value, ...savedSearches].slice(0, 20);
    setSavedSearches(next);
    localStorage.setItem('empathy_saved_knowledge_searches', JSON.stringify(next));
    toast.success('Visualização salva');
  };

  const selectFilter = (prefix: string, value: string) => {
    setQuery(`${prefix}:${value.includes(' ') ? `"${value}"` : value}`);
    setActiveTab('search');
  };

  const importFolder = async () => {
    const sourcePath = await invoke<string | null>('api_select_folder');
    if (!sourcePath) return;
    try {
      const files = await invoke<string[]>('api_import_knowledge_folder', { sourcePath });
      await reindex(true);
      toast.success(`${files.length} documentos importados`, { description: 'Os originais não foram alterados.' });
    } catch (error) {
      toast.error('Falha na importação', { description: String(error) });
    }
  };

  const exportCanvas = async (project?: string) => {
    try {
      const path = await invoke<string>('api_export_json_canvas', { project: project || null });
      toast.success('Canvas criado', { description: path });
    } catch (error) {
      toast.error('Falha ao gerar o Canvas', { description: String(error) });
    }
  };

  const saveContext = async () => {
    try {
      const path = await invoke<string>('api_save_web_context', {
        input: {
          title: context.title, url: context.url, content: context.content,
          project: context.project || null,
          tags: context.tags.split(',').map(value => value.trim()).filter(Boolean),
        },
      });
      setContext({ title: '', url: '', project: '', tags: '', content: '' });
      await reindex(true);
      toast.success('Contexto salvo em Markdown', { description: path });
    } catch (error) {
      toast.error('Não foi possível salvar o contexto', { description: String(error) });
    }
  };

  const discoverExtensions = async () => {
    try {
      const found = await invoke<KnowledgeExtension[]>('api_discover_extensions');
      setExtensions(found);
      toast.success(`${found.length} extensões seguras encontradas`);
    } catch (error) {
      toast.error('Falha ao validar extensões', { description: String(error) });
    }
  };

  const toggleExtension = async (extension: KnowledgeExtension) => {
    try {
      await invoke('api_set_extension_enabled', {
        extensionId: extension.id,
        enabled: !extension.enabled,
      });
      setExtensions(items => items.map(item => item.id === extension.id ? { ...item, enabled: !item.enabled } : item));
    } catch (error) {
      toast.error('Não foi possível alterar a extensão', { description: String(error) });
    }
  };

  const runExtension = async (extension: KnowledgeExtension) => {
    try {
      const output = await invoke<string>('api_run_extension', { extensionId: extension.id });
      await loadDashboard();
      toast.success(`${extension.name} executada`, { description: output });
    } catch (error) {
      toast.error('Falha na automação', { description: String(error) });
    }
  };

  const taskOwners = useMemo(() => {
    const counts = new Map<string, number>();
    dashboard.open_tasks.forEach(task => task.owner && counts.set(task.owner, (counts.get(task.owner) ?? 0) + 1));
    return [...counts.entries()].sort((a, b) => b[1] - a[1]);
  }, [dashboard.open_tasks]);

  const openResult = async (result: SearchResult) => {
    if (result.meeting_id) {
      router.push(`/meeting-details?id=${result.meeting_id}`);
      return;
    }
    try {
      setPreview(await invoke<KnowledgeDocumentContent>('api_read_knowledge_document', { path: result.path }));
    } catch (error) {
      toast.error('Não foi possível abrir o documento', { description: String(error) });
    }
  };

  return (
    <div className="min-h-screen bg-gray-50 px-6 py-8 text-gray-900 dark:bg-gray-950 dark:text-gray-100">
      <div className="mx-auto max-w-7xl">
        <header className="mb-7 flex flex-wrap items-start justify-between gap-4">
          <div>
            <div className="flex items-center gap-2 text-sm font-medium text-blue-600"><Network className="h-4 w-4" /> Memória conectada</div>
            <h1 className="mt-1 text-3xl font-semibold">Conhecimento</h1>
            <p className="mt-2 max-w-2xl text-gray-500">Reuniões, decisões, tarefas, pessoas e projetos derivados dos seus arquivos Markdown.</p>
            {lastIndex && <p className="mt-1 text-xs text-gray-400">Workspace: {lastIndex.root}</p>}
          </div>
          <Button variant="outline" onClick={() => reindex()} disabled={loading}>
            {loading ? <Loader2 className="animate-spin" /> : <RefreshCw />} Reindexar
          </Button>
        </header>

        <Tabs value={activeTab} onValueChange={setActiveTab}>
          <TabsList className="mb-5 flex h-auto flex-wrap justify-start">
            <TabsTrigger value="overview">Visão geral</TabsTrigger>
            <TabsTrigger value="search">Busca global</TabsTrigger>
            <TabsTrigger value="capture">Capturar contexto</TabsTrigger>
            <TabsTrigger value="tools">Importar e exportar</TabsTrigger>
            <TabsTrigger value="extensions">Extensões</TabsTrigger>
          </TabsList>

          <TabsContent value="overview" className="space-y-5">
            <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
              <StatCard icon={BookOpen} label="Documentos" value={dashboard.documents} />
              <StatCard icon={Users} label="Reuniões" value={dashboard.meetings} />
              <StatCard icon={CheckSquare} label="Ações abertas" value={dashboard.open_tasks.length} />
              <StatCard icon={GitBranch} label="Decisões" value={dashboard.recent_decisions.length} />
            </div>
            <div className="grid gap-5 lg:grid-cols-3">
              <ValueCloud title="Projetos" values={dashboard.projects} onSelect={value => selectFilter('project', value)} />
              <ValueCloud title="Participantes" values={dashboard.participants} onSelect={value => selectFilter('person', value)} />
              <ValueCloud title="Tags" values={dashboard.tags} onSelect={value => selectFilter('tag', value)} />
            </div>
            <div className="grid gap-5 lg:grid-cols-2">
              <section className="rounded-xl border bg-white p-5 shadow-sm dark:bg-gray-900">
                <h2 className="mb-4 flex items-center gap-2 font-semibold"><CheckSquare className="h-4 w-4" /> Ações pendentes</h2>
                <div className="max-h-96 space-y-3 overflow-auto">
                  {dashboard.open_tasks.length === 0 && <p className="text-sm text-gray-500">Nenhuma tarefa Markdown pendente.</p>}
                  {dashboard.open_tasks.map(task => (
                    <button key={task.id} onClick={() => task.meeting_id && router.push(`/meeting-details?id=${task.meeting_id}`)}
                      className="block w-full rounded-lg border p-3 text-left hover:bg-gray-50 dark:hover:bg-gray-800">
                      <p className="text-sm">{task.text}</p>
                      <p className="mt-1 text-xs text-gray-400">{task.owner || 'Sem responsável'} · linha {task.line_number}</p>
                    </button>
                  ))}
                </div>
                {taskOwners.length > 0 && <p className="mt-3 text-xs text-gray-400">Por responsável: {taskOwners.map(([owner, count]) => `${owner} (${count})`).join(', ')}</p>}
              </section>
              <section className="rounded-xl border bg-white p-5 shadow-sm dark:bg-gray-900">
                <h2 className="mb-4 flex items-center gap-2 font-semibold"><Sparkles className="h-4 w-4" /> Decisões recentes</h2>
                <div className="max-h-96 space-y-3 overflow-auto">
                  {dashboard.recent_decisions.length === 0 && <p className="text-sm text-gray-500">As seções “Decisões” dos resumos aparecerão aqui.</p>}
                  {dashboard.recent_decisions.map(decision => (
                    <button key={decision.id} onClick={() => decision.meeting_id && router.push(`/meeting-details?id=${decision.meeting_id}`)}
                      className="block w-full rounded-lg border p-3 text-left hover:bg-gray-50 dark:hover:bg-gray-800">
                      <p className="text-sm">{decision.text}</p>
                      <p className="mt-1 text-xs text-gray-400">Linha {decision.line_number}</p>
                    </button>
                  ))}
                </div>
              </section>
            </div>
          </TabsContent>

          <TabsContent value="search">
            <section className="rounded-xl border bg-white p-5 shadow-sm dark:bg-gray-900">
              <div className="relative">
                <Search className="absolute left-3 top-3 h-4 w-4 text-gray-400" />
                <Input className="pl-10" value={query} onChange={event => setQuery(event.target.value)}
                  placeholder="Buscar ou usar project:, person:, tag:, kind:, has:decision…" autoFocus />
              </div>
              <p className="mt-2 text-xs text-gray-400">Exemplo: project:EmpathyIA person:Gabriel has:decision atualização</p>
              <div className="mt-3 flex flex-wrap items-center gap-2">
                <Button size="sm" variant="outline" onClick={saveSearch} disabled={!query.trim()}>Salvar visualização</Button>
                {savedSearches.map(saved => (
                  <span key={saved} className="inline-flex overflow-hidden rounded-full bg-gray-100 text-xs dark:bg-gray-800">
                    <button onClick={() => setQuery(saved)} className="px-3 py-1 hover:bg-gray-200 dark:hover:bg-gray-700">{saved}</button>
                    <button aria-label={`Excluir visualização ${saved}`} onClick={() => {
                      const next = savedSearches.filter(value => value !== saved);
                      setSavedSearches(next);
                      localStorage.setItem('empathy_saved_knowledge_searches', JSON.stringify(next));
                    }} className="border-l px-2 py-1 text-gray-400 hover:text-red-600">×</button>
                  </span>
                ))}
              </div>
              <div className="mt-5 space-y-3">
                {searching && <Loader2 className="mx-auto animate-spin text-gray-400" />}
                {!searching && query && results.length === 0 && <p className="text-center text-sm text-gray-500">Nenhum resultado.</p>}
                {results.map(result => (
                  <button key={result.path} onClick={() => openResult(result)}
                    className="block w-full rounded-lg border p-4 text-left hover:bg-gray-50 dark:hover:bg-gray-800">
                    <div className="flex items-center justify-between gap-4">
                      <h3 className="font-medium">{result.title}</h3>
                      <span className="rounded bg-gray-100 px-2 py-1 text-xs dark:bg-gray-800">{result.kind}</span>
                    </div>
                    {result.project && <p className="mt-1 text-xs text-blue-600">{result.project}</p>}
                    <p className="mt-2 line-clamp-2 text-sm text-gray-500">{result.snippet}</p>
                  </button>
                ))}
              </div>
            </section>
          </TabsContent>

          <TabsContent value="capture">
            <section className="mx-auto max-w-3xl rounded-xl border bg-white p-6 shadow-sm dark:bg-gray-900">
              <h2 className="flex items-center gap-2 text-lg font-semibold"><Globe2 className="h-5 w-5" /> Contexto da web</h2>
              <p className="mb-5 mt-1 text-sm text-gray-500">Cole uma pauta, issue, documentação ou pesquisa. O conteúdo será preservado como Markdown.</p>
              <div className="grid gap-4 sm:grid-cols-2">
                <Input placeholder="Título" value={context.title} onChange={event => setContext({ ...context, title: event.target.value })} />
                <Input placeholder="https://…" value={context.url} onChange={event => setContext({ ...context, url: event.target.value })} />
                <Input placeholder="Projeto (opcional)" value={context.project} onChange={event => setContext({ ...context, project: event.target.value })} />
                <Input placeholder="Tags separadas por vírgula" value={context.tags} onChange={event => setContext({ ...context, tags: event.target.value })} />
              </div>
              <Textarea className="mt-4 min-h-64" placeholder="Conteúdo ou trecho selecionado…" value={context.content}
                onChange={event => setContext({ ...context, content: event.target.value })} />
              <Button className="mt-4" onClick={saveContext}>Salvar contexto</Button>
            </section>
          </TabsContent>

          <TabsContent value="tools">
            <div className="grid gap-5 md:grid-cols-2">
              <section className="rounded-xl border bg-white p-6 shadow-sm dark:bg-gray-900">
                <FileInput className="h-6 w-6 text-blue-600" />
                <h2 className="mt-3 text-lg font-semibold">Importar conhecimento</h2>
                <p className="mb-5 mt-2 text-sm text-gray-500">Importa Vaults e exportações em Markdown, texto, HTML, VTT/SRT, CSV e JSON, preservando os originais e anexos.</p>
                <Button onClick={importFolder}>Selecionar pasta</Button>
              </section>
              <section className="rounded-xl border bg-white p-6 shadow-sm dark:bg-gray-900">
                <Boxes className="h-6 w-6 text-purple-600" />
                <h2 className="mt-3 text-lg font-semibold">JSON Canvas</h2>
                <p className="mb-5 mt-2 text-sm text-gray-500">Gera um mapa visual aberto das reuniões, compatível com ferramentas que usam `.canvas`.</p>
                <div className="flex flex-wrap gap-2">
                  <Button onClick={() => exportCanvas()}>Todas as reuniões</Button>
                  {dashboard.projects.slice(0, 5).map(project => (
                    <Button key={project.value} variant="outline" onClick={() => exportCanvas(project.value)}>{project.value}</Button>
                  ))}
                </div>
              </section>
            </div>
          </TabsContent>

          <TabsContent value="extensions">
            <section className="rounded-xl border bg-white p-6 shadow-sm dark:bg-gray-900">
              <h2 className="text-lg font-semibold">Automações declarativas</h2>
              <p className="mt-2 max-w-3xl text-sm text-gray-500">O Empathy valida manifestos em `.empathy/extensions`. Somente reindexação, Canvas e digest são permitidos; nenhum JavaScript externo é executado.</p>
              <Button className="mt-4" onClick={discoverExtensions}>Validar extensões</Button>
              <div className="mt-5 space-y-3">
                {extensions.map(extension => (
                  <div key={extension.id} className="rounded-lg border p-4">
                    <div className="flex items-center justify-between gap-3"><h3 className="font-medium">{extension.name}</h3><span className="text-xs text-gray-400">{extension.action}</span></div>
                    <p className="mt-1 text-sm text-gray-500">{extension.description || 'Sem descrição'}</p>
                    <div className="mt-3 flex gap-2">
                      <Button size="sm" variant={extension.enabled ? 'secondary' : 'outline'} onClick={() => toggleExtension(extension)}>
                        {extension.enabled ? 'Desativar' : 'Ativar'}
                      </Button>
                      <Button size="sm" disabled={!extension.enabled} onClick={() => runExtension(extension)}>Executar</Button>
                    </div>
                  </div>
                ))}
              </div>
            </section>
          </TabsContent>
        </Tabs>
        <Dialog open={!!preview} onOpenChange={open => !open && setPreview(null)}>
          <DialogContent className="max-h-[85vh] max-w-4xl overflow-y-auto">
            <DialogTitle>{preview?.title}</DialogTitle>
            <p className="break-all text-xs text-gray-400">{preview?.path}</p>
            <article className="prose prose-sm mt-4 max-w-none dark:prose-invert">
              <ReactMarkdown remarkPlugins={[remarkGfm]}>{preview?.content || ''}</ReactMarkdown>
            </article>
          </DialogContent>
        </Dialog>
      </div>
    </div>
  );
}
