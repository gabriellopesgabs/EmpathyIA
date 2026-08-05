'use client';

import React, { useEffect, useRef } from 'react';
import { usePathname } from 'next/navigation';

interface MainContentProps {
  children: React.ReactNode;
}

const MainContent: React.FC<MainContentProps> = ({ children }) => {
  const pathname = usePathname();
  const contentRef = useRef<HTMLElement>(null);

  useEffect(() => {
    contentRef.current?.querySelectorAll<HTMLElement>('.document-scroll, .knowledge-shell, .settings-content')
      .forEach(element => element.scrollTo({ top: 0, left: 0 }));
  }, [pathname]);

  return (
    <main ref={contentRef} className="app-content">
      {children}
    </main>
  );
};

export default MainContent;
