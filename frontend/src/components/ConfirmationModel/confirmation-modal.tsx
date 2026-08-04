import React from 'react';

interface ConfirmationModalProps {
  onConfirm: () => void;
  onArchive?: () => void;
  onCancel: () => void;
  text: string;
  isOpen: boolean;
  isArchived?: boolean;
}

export function ConfirmationModal({
  onConfirm,
  onArchive,
  onCancel,
  text,
  isOpen,
  isArchived = false,
}: ConfirmationModalProps) {
  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50" role="presentation">
      <div className="bg-white dark:bg-gray-900 rounded-xl p-6 max-w-md w-full mx-4 shadow-xl" role="dialog" aria-modal="true" aria-labelledby="note-disposition-title">
        <h2 id="note-disposition-title" className="text-xl font-semibold mb-3 text-gray-900 dark:text-gray-100">
          {isArchived ? 'Excluir nota arquivada?' : 'O que deseja fazer com esta nota?'}
        </h2>
        <p className="text-gray-600 dark:text-gray-300 mb-6">{text}</p>
        <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
          <button
            onClick={onCancel}
            className="px-4 py-2 text-gray-600 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800 rounded-md transition-colors"
          >
            Cancelar
          </button>
          {!isArchived && onArchive && (
            <button
              onClick={onArchive}
              className="px-4 py-2 text-gray-800 dark:text-gray-100 bg-gray-100 dark:bg-gray-800 hover:bg-gray-200 dark:hover:bg-gray-700 rounded-md transition-colors"
            >
              Arquivar em vez disso
            </button>
          )}
          <button
            onClick={onConfirm}
            className="px-4 py-2 bg-red-600 text-white hover:bg-red-700 rounded-md transition-colors"
          >
            Excluir
          </button>
        </div>
      </div>
    </div>
  );
}
