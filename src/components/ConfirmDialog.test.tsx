import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import ConfirmDialog from './ConfirmDialog';

vi.mock('../i18n', () => ({
  useI18n: () => ({ t: { cancel: 'Cancel' } }),
}));

describe('ConfirmDialog', () => {
  it('supports a distinct secondary action without resolving the primary choice', () => {
    const resolve = vi.fn();
    const onSecondary = vi.fn();
    const onClose = vi.fn();
    render(
      <ConfirmDialog
        dialog={{
          title: 'Merge entries',
          message: 'Choose how to merge.',
          confirmLabel: 'Merge only',
          secondaryLabel: 'Merge and delete originals',
          secondaryTone: 'danger',
          onSecondary,
          resolve,
        }}
        onClose={onClose}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Merge and delete originals' }));

    expect(onSecondary).toHaveBeenCalledOnce();
    expect(resolve).not.toHaveBeenCalled();
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('can place the secondary action after the safer primary action', () => {
    const resolve = vi.fn();
    const onSecondary = vi.fn();
    render(
      <ConfirmDialog
        dialog={{
          title: 'Clear history',
          message: 'Choose a scope.',
          confirmLabel: 'Clear current category',
          secondaryLabel: 'Clear all',
          secondaryTone: 'danger',
          secondaryAfterPrimary: true,
          onSecondary,
          resolve,
        }}
        onClose={vi.fn()}
      />,
    );

    expect(screen.getAllByRole('button').map((button) => button.textContent)).toEqual([
      'Cancel',
      'Clear current category',
      'Clear all',
    ]);
  });
});
