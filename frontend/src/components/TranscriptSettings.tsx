import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from './ui/select';
import { Input } from './ui/input';
import { Button } from './ui/button';
import { Label } from './ui/label';
import { Eye, EyeOff, Lock, Unlock, Cpu, Zap, RefreshCw } from 'lucide-react';
import { toast } from 'sonner';
import { ModelManager } from './WhisperModelManager';
import { ParakeetModelManager } from './ParakeetModelManager';


export interface TranscriptModelProps {
    provider: 'localWhisper' | 'parakeet' | 'deepgram' | 'elevenLabs' | 'groq' | 'openai';
    model: string;
    apiKey?: string | null;
}

export interface TranscriptSettingsProps {
    transcriptModelConfig: TranscriptModelProps;
    setTranscriptModelConfig: (config: TranscriptModelProps) => void;
    onModelSelect?: () => void;
}

export function TranscriptSettings({ transcriptModelConfig, setTranscriptModelConfig, onModelSelect }: TranscriptSettingsProps) {
    const [apiKey, setApiKey] = useState<string | null>(transcriptModelConfig.apiKey || null);
    const [showApiKey, setShowApiKey] = useState<boolean>(false);
    const [isApiKeyLocked, setIsApiKeyLocked] = useState<boolean>(true);
    const [isLockButtonVibrating, setIsLockButtonVibrating] = useState<boolean>(false);
    // Hardware benchmark state
    interface HardwareRecommendation {
        cpu_cores: number;
        memory_gb: number;
        gpu_type: string;
        has_gpu_acceleration: boolean;
        performance_tier: string;
        recommended_transcription_engine: string;
        recommended_transcription_model: string;
        recommended_summary_provider: string;
        recommended_summary_model: string;
        max_recommended_context: number;
        explanation: string;
    }
    const [hardwareRec, setHardwareRec] = useState<HardwareRecommendation | null>(null);
    const [isTestingHardware, setIsTestingHardware] = useState<boolean>(false);

    const runHardwareBenchmark = async () => {
        setIsTestingHardware(true);
        try {
            const data = (await invoke('get_hardware_recommendations')) as HardwareRecommendation;
            setHardwareRec(data);
            toast.success('Diagnóstico de hardware concluído!');
        } catch (err) {
            console.error('Falha ao rodar teste de hardware:', err);
        } finally {
            setIsTestingHardware(false);
        }
    };

    useEffect(() => {
        runHardwareBenchmark();
    }, []);

    const [uiProvider, setUiProvider] = useState<TranscriptModelProps['provider']>(transcriptModelConfig.provider);

    // Sync uiProvider when backend config changes (e.g., after model selection or initial load)
    useEffect(() => {
        setUiProvider(transcriptModelConfig.provider);
    }, [transcriptModelConfig.provider]);

    useEffect(() => {
        if (transcriptModelConfig.provider === 'localWhisper' || transcriptModelConfig.provider === 'parakeet') {
            setApiKey(null);
        }
    }, [transcriptModelConfig.provider]);

    const fetchApiKey = async (provider: string) => {
        try {

            const data = await invoke('api_get_transcript_api_key', { provider }) as string;

            setApiKey(data || '');
        } catch (err) {
            console.error('Error fetching API key:', err);
            setApiKey(null);
        }
    };
    const modelOptions = {
        localWhisper: [], // Model selection handled by ModelManager component
        parakeet: [], // Model selection handled by ParakeetModelManager component
        deepgram: ['nova-2-phonecall'],
        elevenLabs: ['eleven_multilingual_v2'],
        groq: ['llama-3.3-70b-versatile'],
        openai: ['gpt-4o'],
    };
    const requiresApiKey = transcriptModelConfig.provider === 'deepgram' || transcriptModelConfig.provider === 'elevenLabs' || transcriptModelConfig.provider === 'openai' || transcriptModelConfig.provider === 'groq';

    const handleInputClick = () => {
        if (isApiKeyLocked) {
            setIsLockButtonVibrating(true);
            setTimeout(() => setIsLockButtonVibrating(false), 500);
        }
    };

    const handleWhisperModelSelect = (modelName: string) => {
        // Always update config when model is selected, regardless of current provider
        // This ensures the model is set when user switches back
        setTranscriptModelConfig({
            ...transcriptModelConfig,
            provider: 'localWhisper', // Ensure provider is set correctly
            model: modelName
        });
        // Close modal after selection
        if (onModelSelect) {
            onModelSelect();
        }
    };

    const handleParakeetModelSelect = (modelName: string) => {
        // Always update config when model is selected, regardless of current provider
        // This ensures the model is set when user switches back
        setTranscriptModelConfig({
            ...transcriptModelConfig,
            provider: 'parakeet', // Ensure provider is set correctly
            model: modelName
        });
        // Close modal after selection
        if (onModelSelect) {
            onModelSelect();
        }
    };

    return (
        <div className="space-y-6">
            {/* Hardware Benchmark & Recommendation Card */}
            <div className="bg-gradient-to-br from-slate-900 to-slate-800 text-white rounded-xl p-4 shadow-md border border-slate-700/80 mb-5">
                <div className="flex items-center justify-between mb-2">
                    <div className="flex items-center gap-2">
                        <div className="p-1.5 rounded-lg bg-blue-500/20 text-blue-400">
                            <Cpu className="w-5 h-5" />
                        </div>
                        <div>
                            <h4 className="font-semibold text-sm">Diagnóstico do Hardware do Mac</h4>
                            <p className="text-xs text-slate-400">Análise de CPU, RAM e GPU para indicar o melhor modelo de Transcrição e LLM</p>
                        </div>
                    </div>
                    <Button
                        onClick={runHardwareBenchmark}
                        disabled={isTestingHardware}
                        size="sm"
                        className="text-xs font-semibold bg-white hover:bg-slate-100 text-slate-900 shadow-sm border border-slate-200"
                    >
                        <RefreshCw className={`w-3.5 h-3.5 mr-1.5 text-blue-600 ${isTestingHardware ? 'animate-spin' : ''}`} />
                        {isTestingHardware ? 'Testando...' : 'Testar Hardware'}
                    </Button>
                </div>

                {hardwareRec && (
                    <div className="mt-3 pt-3 border-t border-slate-700/60 space-y-3">
                        <div className="grid grid-cols-4 gap-2 text-center text-xs">
                            <div className="bg-slate-800/80 p-2 rounded-lg border border-slate-700/40">
                                <span className="text-slate-400 block text-[10px]">CPU Cores</span>
                                <span className="font-bold text-slate-100">{hardwareRec.cpu_cores} núcleos</span>
                            </div>
                            <div className="bg-slate-800/80 p-2 rounded-lg border border-slate-700/40">
                                <span className="text-slate-400 block text-[10px]">GPU</span>
                                <span className="font-bold text-blue-400 truncate block">{hardwareRec.gpu_type}</span>
                            </div>
                            <div className="bg-slate-800/80 p-2 rounded-lg border border-slate-700/40">
                                <span className="text-slate-400 block text-[10px]">Tier Performance</span>
                                <span className="font-bold text-emerald-400">{hardwareRec.performance_tier}</span>
                            </div>
                            <div className="bg-slate-800/80 p-2 rounded-lg border border-slate-700/40">
                                <span className="text-slate-400 block text-[10px]">Aceleração GPU</span>
                                <span className={`font-bold ${hardwareRec.has_gpu_acceleration ? 'text-emerald-400' : 'text-amber-400'}`}>
                                    {hardwareRec.has_gpu_acceleration ? 'Ativa ⚡' : 'CPU Only'}
                                </span>
                            </div>
                        </div>

                        <div className="bg-blue-950/40 border border-blue-800/40 rounded-lg p-3 text-xs leading-relaxed text-blue-200">
                            <div className="flex items-center gap-1.5 font-semibold text-blue-300 mb-1">
                                <Zap className="w-3.5 h-3.5 text-amber-400" />
                                <span>Recomendação Ideal do Sistema:</span>
                            </div>
                            <p className="mb-2 text-slate-300">{hardwareRec.explanation}</p>
                            <div className="flex flex-wrap gap-2 text-[11px]">
                                <span className="bg-blue-900/60 border border-blue-700/50 px-2 py-0.5 rounded text-blue-200">
                                    🎙️ Modelo Transcrição Recomendado: <strong>{hardwareRec.recommended_transcription_engine} ({hardwareRec.recommended_transcription_model})</strong>
                                </span>
                                <span className="bg-emerald-900/60 border border-emerald-700/50 px-2 py-0.5 rounded text-emerald-200">
                                    🧠 LLM Resumo/Agentes Recomendado: <strong>{hardwareRec.recommended_summary_provider} ({hardwareRec.recommended_summary_model})</strong>
                                </span>
                            </div>
                        </div>
                    </div>
                )}
            </div>

            <div>
                <div className="space-y-4 pb-6">
                    <div>
                        <Label className="block text-sm font-medium text-gray-700 mb-1">
                            Transcript Model
                        </Label>
                        <div className="flex space-x-2 mx-1">
                            <Select
                                value={uiProvider}
                                onValueChange={(value) => {
                                    const provider = value as TranscriptModelProps['provider'];
                                    setUiProvider(provider);
                                    if (provider !== 'localWhisper' && provider !== 'parakeet') {
                                        fetchApiKey(provider);
                                    }
                                }}
                            >
                                <SelectTrigger className='focus:ring-1 focus:ring-blue-500 focus:border-blue-500'>
                                    <SelectValue placeholder="Select provider" />
                                </SelectTrigger>
                                <SelectContent>
                                    <SelectItem value="parakeet">⚡ Parakeet (Recommended - Real-time / Accurate)</SelectItem>
                                    <SelectItem value="localWhisper">🏠 Local Whisper (High Accuracy)</SelectItem>
                                    {/* <SelectItem value="deepgram">☁️ Deepgram (Backup)</SelectItem>
                                    <SelectItem value="elevenLabs">☁️ ElevenLabs</SelectItem>
                                    <SelectItem value="groq">☁️ Groq</SelectItem>
                                    <SelectItem value="openai">☁️ OpenAI</SelectItem> */}
                                </SelectContent>
                            </Select>

                            {uiProvider !== 'localWhisper' && uiProvider !== 'parakeet' && (
                                <Select
                                    value={transcriptModelConfig.model}
                                    onValueChange={(value) => {
                                        const model = value as TranscriptModelProps['model'];
                                        setTranscriptModelConfig({ ...transcriptModelConfig, provider: uiProvider, model });
                                    }}
                                >
                                    <SelectTrigger className='focus:ring-1 focus:ring-blue-500 focus:border-blue-500'>
                                        <SelectValue placeholder="Select model" />
                                    </SelectTrigger>
                                    <SelectContent>
                                        {modelOptions[uiProvider].map((model) => (
                                            <SelectItem key={model} value={model}>{model}</SelectItem>
                                        ))}
                                    </SelectContent>
                                </Select>
                            )}

                        </div>
                    </div>

                    {uiProvider === 'localWhisper' && (
                        <div className="mt-6">
                            <ModelManager
                                selectedModel={transcriptModelConfig.provider === 'localWhisper' ? transcriptModelConfig.model : undefined}
                                onModelSelect={handleWhisperModelSelect}
                                autoSave={true}
                            />
                        </div>
                    )}

                    {uiProvider === 'parakeet' && (
                        <div className="mt-6">
                            <ParakeetModelManager
                                selectedModel={transcriptModelConfig.provider === 'parakeet' ? transcriptModelConfig.model : undefined}
                                onModelSelect={handleParakeetModelSelect}
                                autoSave={true}
                            />
                        </div>
                    )}

                    {requiresApiKey && (
                        <div>
                            <Label className="block text-sm font-medium text-gray-700 mb-1">
                                API Key
                            </Label>
                            <div className="relative mx-1">
                                <Input
                                    type={showApiKey ? "text" : "password"}
                                    className={`pr-24 focus:ring-1 focus:ring-blue-500 focus:border-blue-500 ${isApiKeyLocked ? 'bg-gray-100 cursor-not-allowed' : ''
                                        }`}
                                    value={apiKey || ''}
                                    onChange={(e) => setApiKey(e.target.value)}
                                    disabled={isApiKeyLocked}
                                    onClick={handleInputClick}
                                    placeholder="Enter your API key"
                                />
                                {isApiKeyLocked && (
                                    <div
                                        onClick={handleInputClick}
                                        className="absolute inset-0 flex items-center justify-center bg-gray-100 bg-opacity-50 rounded-md cursor-not-allowed"
                                    />
                                )}
                                <div className="absolute inset-y-0 right-0 pr-1 flex items-center">
                                    <Button
                                        type="button"
                                        variant="ghost"
                                        size="icon"
                                        onClick={() => setIsApiKeyLocked(!isApiKeyLocked)}
                                        className={`transition-colors duration-200 ${isLockButtonVibrating ? 'animate-vibrate text-red-500' : ''
                                            }`}
                                        title={isApiKeyLocked ? "Unlock to edit" : "Lock to prevent editing"}
                                    >
                                        {isApiKeyLocked ? <Lock className="h-4 w-4" /> : <Unlock className="h-4 w-4" />}
                                    </Button>
                                    <Button
                                        type="button"
                                        variant="ghost"
                                        size="icon"
                                        onClick={() => setShowApiKey(!showApiKey)}
                                    >
                                        {showApiKey ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                                    </Button>
                                </div>
                            </div>
                        </div>
                    )}
                </div>
            </div>
        </div >
    )
}








