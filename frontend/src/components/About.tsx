import React, { useState, useEffect } from "react";
import { invoke } from '@tauri-apps/api/core';
import { getVersion } from '@tauri-apps/api/app';
import Image from 'next/image';
import { RefreshCw } from 'lucide-react';
import { useUpdates } from './UpdateCheckProvider';


export function About() {
    const [currentVersion, setCurrentVersion] = useState<string>('0.5.0');
    const { checkForUpdates, isChecking } = useUpdates();

    useEffect(() => {
        // Get current version on mount
        getVersion().then(setCurrentVersion).catch(console.error);
    }, []);

    const handleContactClick = async () => {
        try {
            await invoke('open_external_url', { url: 'https://github.com/gabriellopesgabs/EmpathyIA' });
        } catch (error) {
            console.error('Failed to open link:', error);
        }
    };

    return (
        <div className="p-4 space-y-4 h-[80vh] overflow-y-auto">
            {/* Compact Header */}
            <div className="text-center">
                <div className="mb-3">
                    <Image
                        src="icon_128x128.png"
                        alt="Empathy.AI Logo"
                        width={64}
                        height={64}
                        className="mx-auto"
                    />
                </div>
                {/* <h1 className="text-xl font-bold text-gray-900">MyMeet</h1> */}
                <span className="text-sm text-gray-500"> v{currentVersion}</span>
                <div className="mt-2">
                    <button
                        type="button"
                        onClick={() => void checkForUpdates(true)}
                        disabled={isChecking}
                        className="inline-flex items-center gap-2 rounded border border-gray-300 px-3 py-1.5 text-xs font-medium text-gray-700 hover:bg-gray-50 disabled:cursor-not-allowed disabled:opacity-60"
                    >
                        <RefreshCw className={`h-3.5 w-3.5 ${isChecking ? 'animate-spin' : ''}`} />
                        Verificar atualizações
                    </button>
                </div>
                <p className="text-medium text-gray-600 mt-1">
                    Inteligência individual + coletiva + artificial para ampliar o pensamento humano.
                </p>
                
                {/* Privacy-First Banner */}
                <div className="mt-4 max-w-md mx-auto bg-emerald-50/80 border border-emerald-200/80 rounded-xl p-3.5 shadow-sm text-center">
                    <p className="text-xs text-emerald-950 leading-relaxed font-medium">
                        🛡️ Gravações e transcrições ficam no seu computador. Se você escolher um provedor externo para resumos, o texto necessário será enviado a esse provedor com a sua autorização.
                    </p>
                </div>
            </div>

            {/* Features Grid - Compact */}
            <div className="space-y-3">
                <h2 className="text-base font-semibold text-gray-800">O que torna o Empathy.AI diferente</h2>
                <div className="grid grid-cols-2 gap-2">
                    <div className="bg-gray-50 rounded p-3 hover:bg-gray-100 transition-colors">
                        <h3 className="font-bold text-sm text-gray-900 mb-1">Privacidade primeiro</h3>
                        <p className="text-xs text-gray-600 leading-relaxed">Gravação e transcrição podem permanecer no seu computador, sob o seu controle.</p>
                    </div>
                    <div className="bg-gray-50 rounded p-3 hover:bg-gray-100 transition-colors">
                        <h3 className="font-bold text-sm text-gray-900 mb-1">Contribuição humana primeiro</h3>
                        <p className="text-xs text-gray-600 leading-relaxed">Skills sempre criam uma proposta revisável e nunca substituem silenciosamente o que você escreveu.</p>
                    </div>
                    <div className="bg-gray-50 rounded p-3 hover:bg-gray-100 transition-colors">
                        <h3 className="font-bold text-sm text-gray-900 mb-1">Memória coletiva local</h3>
                        <p className="text-xs text-gray-600 leading-relaxed">Conecte reuniões, pessoas, decisões e notas sem depender de uma nuvem.</p>
                    </div>
                    <div className="bg-gray-50 rounded p-3 hover:bg-gray-100 transition-colors">
                        <h3 className="font-bold text-sm text-gray-900 mb-1">IA sob sua decisão</h3>
                        <p className="text-xs text-gray-600 leading-relaxed">Veja provedor e contexto, revise o Markdown e só então incorpore uma nova versão.</p>
                    </div>
                </div>
            </div>

            <div className="bg-blue-50 rounded p-3"><p className="text-s text-blue-800"><span className="font-bold">Humano aumentado:</span> a IA amplia reflexão e memória compartilhada; a autoria e a decisão continuam humanas.</p></div>

            {/* CTA Section - Compact */}
            <div className="text-center space-y-2">
                <h3 className="text-medium font-semibold text-gray-800">Quer adaptar o EmpathyIA ao seu trabalho?</h3>
                <p className="text-s text-gray-600">
                    Acompanhe o projeto, envie sugestões e relate problemas no repositório oficial.
                </p>
                <button
                    onClick={handleContactClick}
                    className="inline-flex items-center px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium rounded transition-colors duration-200 shadow-sm hover:shadow-md"
                >
                    Abrir o projeto EmpathyIA
                </button>
            </div>

            {/* Footer - Compact */}
            <div className="pt-2 border-t border-gray-200 text-center">
                <p className="text-xs text-gray-400">
                    EmpathyIA é software livre derivado do Meetily, com atribuição preservada na licença.
                </p>
            </div>

        </div>

    )
}
