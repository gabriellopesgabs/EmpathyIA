const byId = (id) => document.getElementById(id);

const yamlString = (value) => JSON.stringify(value ?? '');
const slugify = (value) => value.toLowerCase().normalize('NFD')
  .replace(/[\u0300-\u036f]/g, '').replace(/[^a-z0-9]+/g, '-')
  .replace(/^-|-$/g, '').slice(0, 80) || 'contexto';

async function pageContext() {
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  if (!tab?.id) return null;
  const [{ result }] = await chrome.scripting.executeScript({
    target: { tabId: tab.id },
    func: () => ({
      title: document.title,
      url: location.href,
      content: window.getSelection()?.toString().trim()
        || document.querySelector('main, article')?.innerText?.trim()
        || document.body.innerText.trim(),
    }),
  });
  return result;
}

let capturedUrl = '';
pageContext().then((context) => {
  if (!context) return;
  byId('title').value = context.title;
  byId('content').value = context.content.slice(0, 100000);
  capturedUrl = context.url;
});

byId('save').addEventListener('click', async () => {
  const title = byId('title').value.trim();
  if (!title || !capturedUrl) {
    byId('status').textContent = 'Não foi possível ler esta página.';
    return;
  }
  const project = byId('project').value.trim();
  const tags = byId('tags').value.split(',').map((tag) => tag.trim()).filter(Boolean);
  const markdown = `---\nempathy_schema: 2\ntype: context\ntitle: ${yamlString(title)}\nsource_url: ${yamlString(capturedUrl)}\ncaptured_at: ${yamlString(new Date().toISOString())}\nproject: ${yamlString(project)}\ntags: ${JSON.stringify(tags.length ? tags : ['contexto'])}\n---\n\n# ${title}\n\nFonte: [${capturedUrl}](${capturedUrl})\n\n${byId('content').value.trim()}\n`;
  const url = `data:text/markdown;charset=utf-8,${encodeURIComponent(markdown)}`;
  await chrome.downloads.download({ url, filename: `EmpathyIA/${slugify(title)}.md`, saveAs: false });
  byId('status').textContent = 'Markdown salvo em Downloads/EmpathyIA.';
});
