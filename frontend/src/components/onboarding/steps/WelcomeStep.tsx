import React from 'react';
import Image from 'next/image';
import { Lock, Sparkles, Cpu } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { OnboardingContainer } from '../OnboardingContainer';
import { useOnboarding } from '@/contexts/OnboardingContext';

export function WelcomeStep() {
  const { goNext } = useOnboarding();

  const features = [
    {
      icon: Lock,
      title: 'Inteligência individual: registre e desenvolva seu pensamento',
    },
    {
      icon: Sparkles,
      title: 'Inteligência coletiva: conecte pessoas, decisões e memória local',
    },
    {
      icon: Cpu,
      title: 'Inteligência artificial: aplique Skills sob sua decisão',
    },
  ];

  return (
    <OnboardingContainer
      title="Amplie o pensamento humano"
      description="Inteligência individual + coletiva + artificial."
      step={1}
      hideProgress={true}
    >
      <div className="flex flex-col items-center space-y-10">
        <Image
          src="/brand/empathy-lettering-black.svg"
          alt="Empathy — Tech Agency, Human Inside."
          width={220}
          height={54}
          className="h-auto w-[220px]"
          priority
        />

        {/* Divider */}
        <div className="w-16 h-px bg-gray-300" />

        {/* Disclaimer de Privacidade Destacado */}
        <div className="w-full max-w-md bg-emerald-50/80 border border-emerald-200/80 rounded-xl p-4 shadow-sm text-center">
          <div className="flex items-center justify-center gap-2 mb-1.5 text-emerald-800 font-semibold text-sm">
            <Lock className="w-4 h-4 text-emerald-600" />
            <span>Privacy-First por Princípio</span>
          </div>
          <p className="text-xs text-emerald-900 leading-relaxed font-medium">
            Seus arquivos pertencem a você. Modelos locais mantêm o conteúdo no dispositivo; antes de usar um provedor externo, o Empathy mostra exatamente qual contexto será enviado.
          </p>
        </div>

        {/* Features Card */}
        <div className="w-full max-w-md bg-white rounded-lg border border-gray-200 shadow-sm p-6 space-y-4">
          {features.map((feature, index) => {
            const Icon = feature.icon;
            return (
              <div key={index} className="flex items-start gap-3">
                <div className="flex-shrink-0 mt-0.5">
                  <div className="w-5 h-5 rounded-full bg-gray-100 flex items-center justify-center">
                    <Icon className="w-3 h-3 text-gray-700" />
                  </div>
                </div>
                <p className="text-sm text-gray-700 leading-relaxed">{feature.title}</p>
              </div>
            );
          })}
        </div>

        {/* CTA Section */}
        <div className="w-full max-w-xs space-y-3">
          <Button
            onClick={goNext}
            className="w-full h-11 bg-gray-900 hover:bg-gray-800 text-white"
          >
            Começar
          </Button>
          <p className="text-xs text-center text-gray-500">Você mantém a decisão final em cada Skill</p>
        </div>
      </div>
    </OnboardingContainer>
  );
}
