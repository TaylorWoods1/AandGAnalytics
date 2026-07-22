import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import FilterBar from './FilterBar';
import type { MetricsFilter } from '../lib/tauri';

describe('FilterBar', () => {
  it('emits filter updates when dates change', () => {
    const onChange = vi.fn();
    const value: MetricsFilter = {
      project_keys: null,
      from: null,
      to: null,
      issue_types: null,
      assignee_ids: null,
    };

    render(<FilterBar value={value} onChange={onChange} />);

    fireEvent.change(screen.getByLabelText(/from/i), {
      target: { value: '2026-01-01' },
    });

    expect(onChange).toHaveBeenCalledWith(expect.objectContaining({ from: '2026-01-01' }));
  });
});
