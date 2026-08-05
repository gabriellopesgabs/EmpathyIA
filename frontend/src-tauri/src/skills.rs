//! File-first, user-controlled AI skills. Runs are transient: only the UI can
//! persist a reviewed result into a Markdown note.
use crate::database::repositories::setting::SettingsRepository;
use crate::state::AppState;
use crate::summary::llm_client::{generate_summary, LLMProvider};
use dashmap::DashMap;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_dialog::DialogExt;
use tokio_util::sync::CancellationToken;

const SCHEMA: u32 = 1;
static RUNS: Lazy<DashMap<String, CancellationToken>> = Lazy::new(DashMap::new);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SkillLayer {
    Individual,
    Collective,
    Artificial,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillContextPermissions {
    #[serde(default = "yes")]
    pub selection: bool,
    #[serde(default = "yes")]
    pub note: bool,
    #[serde(default)]
    pub transcript: bool,
    #[serde(default)]
    pub related_notes: bool,
}
fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDefinition {
    pub schema: u32,
    pub id: String,
    pub name: String,
    pub description: String,
    pub layer: SkillLayer,
    pub instruction: String,
    pub default_title: String,
    pub context: SkillContextPermissions,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillInfo {
    #[serde(flatten)]
    pub definition: SkillDefinition,
    pub native: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedSkillDocument {
    pub id: String,
    pub title: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillContextRequest {
    pub note_id: String,
    pub note_title: String,
    pub note: String,
    pub selection: Option<String>,
    pub transcript: Option<String>,
    #[serde(default)]
    pub related_notes: Vec<RelatedSkillDocument>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillRunResult {
    pub run_id: String,
    pub skill_id: String,
    pub skill_name: String,
    pub layer: SkillLayer,
    pub title: String,
    pub markdown: String,
    pub provider: String,
    pub model: String,
    pub source_scope: String,
    pub external: bool,
    pub context_documents: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillResultMetadata {
    pub id: String,
    pub skill_id: String,
    pub skill_name: String,
    pub layer: SkillLayer,
    pub created_at: String,
    pub source_scope: String,
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub context_documents: Vec<String>,
}

#[derive(Clone, Serialize)]
struct SkillProgress {
    run_id: String,
    status: String,
    message: String,
}

fn skill(
    id: &str,
    name: &str,
    description: &str,
    layer: SkillLayer,
    title: &str,
    instruction: &str,
    transcript: bool,
    related_notes: bool,
) -> SkillDefinition {
    SkillDefinition {
        schema: SCHEMA,
        id: id.into(),
        name: name.into(),
        description: description.into(),
        layer,
        instruction: instruction.into(),
        default_title: title.into(),
        context: SkillContextPermissions {
            selection: true,
            note: true,
            transcript,
            related_notes,
        },
    }
}

fn native_skills() -> Vec<SkillDefinition> {
    use SkillLayer::*;
    vec![
        skill("clarify-thinking", "Clarificar pensamento", "Organiza uma ideia preservando ambiguidades importantes.", Individual, "Pensamento clarificado", "Clarifique o raciocínio, explicite premissas, pontos centrais e questões abertas. Preserve a voz e não invente fatos.", false, false),
        skill("socratic-questions", "Perguntas socráticas", "Cria perguntas para aprofundar o pensamento.", Individual, "Perguntas para aprofundar", "Crie perguntas socráticas específicas, abertas e não indutivas. Não responda por quem escreveu.", false, false),
        skill("counterpoints", "Contrapontos", "Expõe alternativas e tensões construtivas.", Individual, "Contrapontos", "Apresente contrapontos fortes, pressupostos frágeis e interpretações alternativas, separando evidência de hipótese.", false, true),
        skill("personal-next-step", "Próximo passo pessoal", "Converte reflexão em uma ação concreta.", Individual, "Próximo passo pessoal", "Proponha um próximo passo pequeno e verificável, coerente com a intenção expressa, e uma pergunta de reflexão.", false, false),
        skill("consensus-divergence", "Consensos e divergências", "Evidencia acordos, tensões e pontos sem voz suficiente.", Collective, "Consensos e divergências", "Identifique consensos, divergências, incertezas e tópicos não resolvidos. Atribua falas somente com evidência.", true, true),
        skill("perspective-synthesis", "Síntese de perspectivas", "Integra pontos de vista sem homogeneizá-los.", Collective, "Síntese de perspectivas", "Sintetize perspectivas distintas, preserve diferenças e mostre onde se complementam ou contradizem.", true, true),
        skill("voices-contributions", "Vozes e contribuições", "Reconhece as contribuições presentes.", Collective, "Vozes e contribuições", "Mapeie contribuições por participante somente com atribuição disponível. Não especule identidades.", true, false),
        skill("connect-memory", "Conectar com a memória", "Relaciona a nota à memória local escolhida.", Collective, "Conexões com a memória", "Compare os documentos escolhidos. Mostre continuidade, contradições e decisões anteriores, citando o título das fontes.", true, true),
        skill("structured-summary", "Resumo estruturado", "Resume preservando decisões e nuances.", Artificial, "Resumo estruturado", "Produza um resumo conciso com contexto, pontos principais, decisões, dúvidas e próximos passos. Omita seções vazias.", true, true),
        skill("meeting-summary", "Resumo da reunião", "Transforma uma transcrição em nota revisável.", Artificial, "Resumo da reunião", "Produza um resumo fiel com temas, decisões, compromissos, responsáveis quando explícitos e questões abertas.", true, true),
        skill("decisions-commitments", "Decisões e compromissos", "Extrai decisões, responsáveis e prazos confirmados.", Artificial, "Decisões e compromissos", "Liste decisões e compromissos. Diferencie confirmado, proposto e pendente. Não invente responsáveis ou prazos.", true, true),
        skill("risks-gaps", "Riscos e lacunas", "Localiza riscos, dependências e informação ausente.", Artificial, "Riscos e lacunas", "Identifique riscos, lacunas, dependências e contradições. Classifique cada item como evidência ou hipótese.", true, true),
        skill("action-plan", "Plano de ação", "Converte conteúdo em um plano verificável.", Artificial, "Plano de ação", "Crie ações claras com resultado esperado, responsável e prazo somente quando explícitos e critério de conclusão.", true, true),
        skill("software-architect", "Perspectiva: Arquitetura", "Analisa decisões técnicas e trade-offs.", Artificial, "Leitura de arquitetura", "Analise arquitetura, decisões técnicas, trade-offs, dependências e dívida técnica.", true, true),
        skill("agile-coach", "Perspectiva: Agilidade", "Analisa dinâmica, fluxo e bloqueios.", Collective, "Leitura de agilidade", "Analise dinâmica do time, fluxo, bloqueios, feedback e oportunidades de melhoria.", true, true),
        skill("project-manager", "Perspectiva: Projeto", "Analisa marcos, dependências e prazos.", Collective, "Leitura de projeto", "Analise marcos, ações, dependências, responsáveis e prazos sem completar dados ausentes.", true, true),
        skill("ux-designer", "Perspectiva: Experiência", "Analisa necessidades, usabilidade e feedback.", Collective, "Leitura de experiência", "Analise necessidades das pessoas, experiência, usabilidade, decisões de design e feedback observado.", true, true),
        // Compatibility: the nine former summary templates are now ordinary,
        // append-only Skills and no longer replace the note's human content.
        skill("agenda-takeaways", "Pauta e aprendizados", "Compara pauta planejada e conversa realizada.", Artificial, "Pauta e aprendizados", "Compare a pauta mencionada com os assuntos discutidos e destaque aprendizados, desvios e pendências.", true, false),
        skill("business-manager", "Leitura estratégica", "Organiza decisões, indicadores, cronograma e riscos.", Artificial, "Leitura estratégica", "Organize decisões de negócio, objetivos, indicadores explicitamente citados, cronograma e riscos de mercado.", true, true),
        skill("daily-standup", "Daily stand-up", "Estrutura atualizações curtas do time.", Collective, "Daily stand-up", "Organize por progresso, próximo passo e bloqueios. Atribua pessoas somente quando a transcrição permitir.", true, false),
        skill("deep-analysis", "Análise profunda", "Investiga questões, riscos, bloqueios e participação.", Artificial, "Análise profunda", "Analise questões não resolvidas, riscos, bloqueios, cobertura da pauta e distribuição de participação sem inferir identidades.", true, true),
        skill("project-sync", "Sincronização de projeto", "Consolida marcos, estado e riscos do projeto.", Collective, "Sincronização de projeto", "Consolide estado, marcos, mudanças, dependências, riscos e próximos checkpoints com fidelidade às fontes.", true, true),
        skill("psychiatric-session", "Registro clínico SOAP", "Organiza conteúdo clínico em SOAP sem substituir julgamento profissional.", Artificial, "Registro clínico SOAP", "Organize somente informações explícitas em formato SOAP. Marque ausências e incertezas; não diagnostique, prescreva ou invente dados. O texto exige revisão profissional.", true, false),
        skill("retrospective", "Retrospectiva", "Estrutura aprendizados e experimentos de melhoria.", Collective, "Retrospectiva", "Organize o que funcionou, o que dificultou, aprendizados e experimentos de melhoria com responsáveis apenas quando explícitos.", true, true),
        skill("client-sales-call", "Conversa com cliente", "Registra objetivos, necessidades e próximos passos.", Collective, "Conversa com cliente", "Organize objetivos, necessidades, objeções, entregáveis e próximos passos. Separe afirmações do cliente de interpretações internas.", true, true),
        skill("standard-meeting", "Nota de reunião", "Estrutura resultados e ações de uma reunião geral.", Artificial, "Nota de reunião", "Crie uma nota geral concisa com contexto, temas, resultados, ações e questões em aberto. Não inclua seções vazias.", true, true),
    ]
}

fn validate(skill: &SkillDefinition) -> Result<(), String> {
    if skill.schema != SCHEMA {
        return Err(format!("Schema incompatível: {}", skill.schema));
    }
    if skill.id.is_empty()
        || !skill
            .id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err("O id deve usar letras minúsculas, números e hífens".into());
    }
    if skill.name.trim().is_empty()
        || skill.description.trim().is_empty()
        || skill.instruction.trim().is_empty()
        || skill.default_title.trim().is_empty()
    {
        return Err("Nome, descrição, instrução e título são obrigatórios".into());
    }
    if skill.instruction.len() > 12_000 {
        return Err("A instrução excede 12.000 caracteres".into());
    }
    Ok(())
}

fn directory<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    let path = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("skills");
    fs::create_dir_all(&path).map_err(|e| format!("Não foi possível criar a biblioteca: {e}"))?;
    Ok(path)
}

fn parse_definition(raw: &str) -> Result<SkillDefinition, String> {
    let skill: SkillDefinition =
        serde_json::from_str(raw).map_err(|error| format!("Skill inválida: {error}"))?;
    validate(&skill)?;
    Ok(skill)
}

fn custom_skills<R: Runtime>(app: &AppHandle<R>) -> Result<Vec<SkillDefinition>, String> {
    let mut skills = Vec::new();
    for entry in fs::read_dir(directory(app)?).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().and_then(|v| v.to_str()) != Some("json") {
            continue;
        }
        let parsed = fs::read_to_string(&path)
            .map_err(|error| error.to_string())
            .and_then(|raw| parse_definition(&raw));
        match parsed {
            Ok(skill) => skills.push(skill),
            _ => log::warn!("Ignoring invalid custom Skill: {}", path.display()),
        }
    }
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(skills)
}

fn find<R: Runtime>(app: &AppHandle<R>, id: &str) -> Result<(SkillDefinition, bool), String> {
    if let Some(value) = custom_skills(app)?.into_iter().find(|s| s.id == id) {
        return Ok((value, false));
    }
    native_skills()
        .into_iter()
        .find(|s| s.id == id)
        .map(|s| (s, true))
        .ok_or_else(|| "Skill não encontrada".into())
}

#[tauri::command]
pub async fn api_list_skills<R: Runtime>(app: AppHandle<R>) -> Result<Vec<SkillInfo>, String> {
    let mut result: Vec<_> = native_skills()
        .into_iter()
        .map(|definition| SkillInfo {
            definition,
            native: true,
        })
        .collect();
    result.extend(
        custom_skills(&app)?
            .into_iter()
            .map(|definition| SkillInfo {
                definition,
                native: false,
            }),
    );
    Ok(result)
}

#[tauri::command]
pub async fn api_get_skill<R: Runtime>(
    app: AppHandle<R>,
    skill_id: String,
) -> Result<SkillInfo, String> {
    let (definition, native) = find(&app, &skill_id)?;
    Ok(SkillInfo { definition, native })
}

#[tauri::command]
pub async fn api_save_custom_skill<R: Runtime>(
    app: AppHandle<R>,
    skill: SkillDefinition,
) -> Result<SkillInfo, String> {
    validate(&skill)?;
    if native_skills().iter().any(|s| s.id == skill.id) {
        return Err("Duplique a Skill nativa com um novo id".into());
    }
    let json = serde_json::to_string_pretty(&skill).map_err(|e| e.to_string())?;
    fs::write(
        directory(&app)?.join(format!("{}.json", skill.id)),
        format!("{json}\n"),
    )
    .map_err(|e| e.to_string())?;
    Ok(SkillInfo {
        definition: skill,
        native: false,
    })
}

#[tauri::command]
pub async fn api_delete_custom_skill<R: Runtime>(
    app: AppHandle<R>,
    skill_id: String,
) -> Result<(), String> {
    if native_skills().iter().any(|s| s.id == skill_id) {
        return Err("Skills nativas são imutáveis".into());
    }
    let path = directory(&app)?.join(format!("{skill_id}.json"));
    if path.exists() {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn api_import_skill<R: Runtime>(
    app: AppHandle<R>,
    file_path: Option<String>,
) -> Result<SkillInfo, String> {
    let path = match file_path {
        Some(path) => PathBuf::from(path),
        None => app
            .dialog()
            .file()
            .add_filter("Empathy Skill", &["json"])
            .blocking_pick_file()
            .map(|v| PathBuf::from(v.to_string()))
            .ok_or_else(|| "Importação cancelada".to_string())?,
    };
    let mut skill = parse_definition(&fs::read_to_string(path).map_err(|e| e.to_string())?)?;
    if native_skills().iter().any(|s| s.id == skill.id) {
        skill.id = format!("{}-custom", skill.id);
    }
    api_save_custom_skill(app, skill).await
}

#[tauri::command]
pub async fn api_export_skill<R: Runtime>(
    app: AppHandle<R>,
    skill_id: String,
    file_path: Option<String>,
) -> Result<String, String> {
    let (skill, _) = find(&app, &skill_id)?;
    let path = match file_path {
        Some(path) => PathBuf::from(path),
        None => app
            .dialog()
            .file()
            .set_file_name(format!("{}.json", skill.id))
            .add_filter("Empathy Skill", &["json"])
            .blocking_save_file()
            .map(|v| PathBuf::from(v.to_string()))
            .ok_or_else(|| "Exportação cancelada".to_string())?,
    };
    fs::write(
        &path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&skill).map_err(|e| e.to_string())?
        ),
    )
    .map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

/// Explicit, idempotent adapter for user-authored legacy templates. Originals
/// remain in templates/ and an existing custom Skill is never overwritten.
#[tauri::command]
pub async fn api_migrate_custom_templates<R: Runtime>(app: AppHandle<R>) -> Result<usize, String> {
    let templates = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("templates");
    if !templates.exists() {
        return Ok(0);
    }
    let mut migrated = 0;
    for entry in fs::read_dir(templates).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().and_then(|v| v.to_str()) != Some("json") {
            continue;
        }
        let value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        let base = path
            .file_stem()
            .and_then(|v| v.to_str())
            .unwrap_or("template")
            .to_ascii_lowercase()
            .replace('_', "-");
        let id = format!("legacy-{base}");
        let destination = directory(&app)?.join(format!("{id}.json"));
        if destination.exists() {
            continue;
        }
        let sections = value
            .get("sections")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let instruction = sections
            .iter()
            .map(|section| {
                format!(
                    "- {}: {}",
                    section
                        .get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Seção"),
                    section
                        .get("instruction")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Organize o conteúdo desta seção.")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let definition = SkillDefinition {
            schema: SCHEMA,
            id,
            name: value
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Template adaptado")
                .into(),
            description: value
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("Skill adaptada de um template personalizado.")
                .into(),
            layer: SkillLayer::Artificial,
            instruction: format!(
                "Produza Markdown seguindo estas seções quando houver evidência:\n{instruction}"
            ),
            default_title: value
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Resultado")
                .into(),
            context: SkillContextPermissions {
                selection: true,
                note: true,
                transcript: true,
                related_notes: false,
            },
        };
        validate(&definition)?;
        fs::write(
            destination,
            format!(
                "{}\n",
                serde_json::to_string_pretty(&definition).map_err(|e| e.to_string())?
            ),
        )
        .map_err(|e| e.to_string())?;
        migrated += 1;
    }
    Ok(migrated)
}

fn emit<R: Runtime>(app: &AppHandle<R>, run_id: &str, status: &str, message: &str) {
    let _ = app.emit(
        "skill-progress",
        SkillProgress {
            run_id: run_id.into(),
            status: status.into(),
            message: message.into(),
        },
    );
}

fn clean_markdown(value: String) -> String {
    value
        .trim()
        .trim_start_matches("```markdown")
        .trim_start_matches("```md")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim()
        .to_string()
}

#[tauri::command]
pub async fn api_run_skill<R: Runtime>(
    app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    skill_id: String,
    context: SkillContextRequest,
) -> Result<SkillRunResult, String> {
    let (skill, _) = find(&app, &skill_id)?;
    if context.related_notes.len() > 5 {
        return Err("Escolha no máximo cinco notas relacionadas".into());
    }
    if !skill.context.transcript
        && context
            .transcript
            .as_deref()
            .is_some_and(|v| !v.trim().is_empty())
    {
        return Err("Esta Skill não permite usar a transcrição".into());
    }
    if !skill.context.related_notes && !context.related_notes.is_empty() {
        return Err("Esta Skill não permite usar notas relacionadas".into());
    }
    let selection = context
        .selection
        .as_deref()
        .filter(|v| !v.trim().is_empty());
    let source = selection.unwrap_or(&context.note);
    if source.trim().is_empty() {
        return Err("Não há conteúdo para processar".into());
    }

    let config = SettingsRepository::get_model_config(state.db_manager.pool())
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Configure um modelo antes de executar uma Skill".to_string())?;
    let provider = LLMProvider::from_str(&config.provider)?;
    let custom = if provider == LLMProvider::CustomOpenAI {
        SettingsRepository::get_custom_openai_config(state.db_manager.pool())
            .await
            .map_err(|e| e.to_string())?
    } else {
        None
    };
    let api_key = if provider == LLMProvider::CustomOpenAI {
        custom
            .as_ref()
            .and_then(|v| v.api_key.clone())
            .unwrap_or_default()
    } else {
        SettingsRepository::get_api_key(state.db_manager.pool(), &config.provider)
            .await
            .map_err(|e| e.to_string())?
            .unwrap_or_default()
    };
    let external = !matches!(provider, LLMProvider::BuiltInAI | LLMProvider::Ollama);
    let run_id = uuid::Uuid::new_v4().to_string();
    let token = CancellationToken::new();
    RUNS.insert(run_id.clone(), token.clone());
    emit(
        &app,
        &run_id,
        "processing",
        "Preparando o contexto escolhido",
    );

    let mut prompt = format!(
        "<primary_document title=\"{}\">\n{}\n</primary_document>",
        context.note_title, source
    );
    let mut documents = vec![context.note_id.clone()];
    if let Some(transcript) = context.transcript.filter(|v| !v.trim().is_empty()) {
        prompt.push_str(&format!("\n\n<transcript>\n{}\n</transcript>", transcript));
        documents.push(format!("{}:transcript", context.note_id));
    }
    for document in &context.related_notes {
        prompt.push_str(&format!(
            "\n\n<related_document id=\"{}\" title=\"{}\">\n{}\n</related_document>",
            document.id, document.title, document.content
        ));
        documents.push(document.id.clone());
    }
    prompt.push_str("\n\nReturn only the proposed Markdown body. Do not add a title, signature, metadata comment, or code fence.");
    let system = format!("You are an Empathy Skill in a human-controlled augmented-intelligence workspace. Human content is canonical. Never invent facts, people, owners, dates or consensus. Distinguish evidence, inference and open questions.\n\nSkill instruction:\n{}", skill.instruction);
    emit(
        &app,
        &run_id,
        "generating",
        "A Skill está elaborando uma proposta",
    );
    let app_data = app.path().app_data_dir().ok();
    let generated = generate_summary(
        &reqwest::Client::new(),
        &provider,
        &config.model,
        &api_key,
        &system,
        &prompt,
        config.ollama_endpoint.as_deref(),
        custom.as_ref().map(|v| v.endpoint.as_str()),
        custom.as_ref().and_then(|v| v.max_tokens.map(|n| n as u32)),
        custom.as_ref().and_then(|v| v.temperature),
        custom.as_ref().and_then(|v| v.top_p),
        app_data.as_ref(),
        None,
        None,
        Some(&token),
    )
    .await;
    RUNS.remove(&run_id);
    match generated {
        Ok(markdown) => {
            emit(&app, &run_id, "completed", "Proposta pronta para revisão");
            Ok(SkillRunResult {
                run_id,
                skill_id: skill.id,
                skill_name: skill.name,
                layer: skill.layer,
                title: skill.default_title,
                markdown: clean_markdown(markdown),
                provider: config.provider,
                model: config.model,
                source_scope: if selection.is_some() {
                    "selection"
                } else {
                    "note"
                }
                .into(),
                external,
                context_documents: documents,
            })
        }
        Err(error) => {
            emit(
                &app,
                &run_id,
                if token.is_cancelled() {
                    "cancelled"
                } else {
                    "error"
                },
                &error,
            );
            Err(error)
        }
    }
}

#[tauri::command]
pub async fn api_cancel_skill(run_id: String) -> Result<bool, String> {
    if let Some(token) = RUNS.get(&run_id) {
        token.cancel();
        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn library_covers_triad() {
        let s = native_skills();
        assert!(s.iter().any(|v| v.layer == SkillLayer::Individual));
        assert!(s.iter().any(|v| v.layer == SkillLayer::Collective));
        assert!(s.iter().any(|v| v.layer == SkillLayer::Artificial));
        assert!(s.iter().all(|v| validate(v).is_ok()));
    }
    #[test]
    fn unsafe_id_is_rejected() {
        let mut s = native_skills().remove(0);
        s.id = "Unsafe/Path".into();
        assert!(validate(&s).is_err());
    }
    #[test]
    fn invalid_skill_file_is_rejected() {
        assert!(parse_definition(r#"{"schema":1,"id":"incomplete"}"#).is_err());
        assert!(parse_definition("not json").is_err());
    }
}
