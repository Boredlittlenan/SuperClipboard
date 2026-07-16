import { useCallback, useState } from 'react';
import type { DragEvent } from 'react';
import { importDroppedImage, importDroppedText } from '../api/clipboard';

const MAX_IMAGE_FILE_BYTES = 20 * 1024 * 1024;
const IMAGE_FILE_PATTERN = /\.(png|jpe?g|gif|webp)$/i;

interface Options {
  onComplete: (inserted: boolean) => void;
  onError: () => void;
}

function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return Boolean(target.closest('input, textarea, [contenteditable="true"]'));
}

function isImageFile(file: File): boolean {
  return file.type.startsWith('image/') || IMAGE_FILE_PATTERN.test(file.name);
}

function hasSupportedPayload(dataTransfer: DataTransfer): boolean {
  const types = Array.from(dataTransfer.types, (type) => type.toLowerCase());
  return dataTransfer.files.length > 0
    || types.includes('files')
    || types.includes('text/plain')
    || types.includes('text/uri-list');
}

function getImageFiles(dataTransfer: DataTransfer): File[] {
  return Array.from(dataTransfer.files).filter(isImageFile);
}

function readImageFile(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = () => reject(reader.error ?? new Error('Failed to read dropped image.'));
    reader.readAsDataURL(file);
  });
}

/**
 * Imports text and image files dropped on the app surface. Editable fields are
 * intentionally excluded so memo and clipboard editors retain native drop behavior.
 */
export function useClipboardDropImport({ onComplete, onError }: Options) {
  const [isDragActive, setIsDragActive] = useState(false);
  const [isImporting, setIsImporting] = useState(false);

  const shouldHandle = useCallback((event: DragEvent<HTMLDivElement>) => (
    !isEditableTarget(event.target) && hasSupportedPayload(event.dataTransfer)
  ), []);

  const handleDragEnter = useCallback((event: DragEvent<HTMLDivElement>) => {
    if (!shouldHandle(event)) return;
    event.preventDefault();
    setIsDragActive(true);
  }, [shouldHandle]);

  const handleDragOver = useCallback((event: DragEvent<HTMLDivElement>) => {
    if (!shouldHandle(event)) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = 'copy';
    setIsDragActive(true);
  }, [shouldHandle]);

  const handleDragLeave = useCallback((event: DragEvent<HTMLDivElement>) => {
    const nextTarget = event.relatedTarget;
    if (nextTarget instanceof Node && event.currentTarget.contains(nextTarget)) return;
    setIsDragActive(false);
  }, []);

  const handleDrop = useCallback(async (event: DragEvent<HTMLDivElement>) => {
    if (!shouldHandle(event)) return;
    event.preventDefault();
    setIsDragActive(false);
    setIsImporting(true);

    try {
      const { dataTransfer } = event;
      const imageFiles = getImageFiles(dataTransfer);
      let inserted = false;

      if (imageFiles.length > 0) {
        for (const file of imageFiles) {
          if (file.size > MAX_IMAGE_FILE_BYTES) {
            throw new Error('Dropped image exceeds the size limit.');
          }
          const dataUrl = await readImageFile(file);
          inserted = (await importDroppedImage(dataUrl)) || inserted;
        }
      } else {
        const text = dataTransfer.getData('text/plain') || dataTransfer.getData('text/uri-list');
        if (!text.trim()) return;
        inserted = await importDroppedText(text);
      }

      onComplete(inserted);
    } catch (error) {
      console.error('Failed to import dropped clipboard content:', error);
      onError();
    } finally {
      setIsImporting(false);
    }
  }, [onComplete, onError, shouldHandle]);

  return {
    isDragActive,
    isImporting,
    handleDragEnter,
    handleDragOver,
    handleDragLeave,
    handleDrop,
  };
}
