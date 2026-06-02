import { fireEvent, screen, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import '../../test/mockDefaultSkillStatusHooks';
import { renderWithProviders } from '../../test/test-utils';
import Skills from '../Skills';

let composioRefresh = vi.fn();
let composioError: string | null = null;
let composioToolkits: string[] = [];
let composioConnectionByToolkit = new Map();

vi.mock('../../hooks/useChannelDefinitions', () => ({
  useChannelDefinitions: () => ({ definitions: [], loading: false, error: null }),
}));

vi.mock('../../lib/skills/skillsApi', () => ({
  installSkill: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('../../lib/skills/hooks', () => ({
  useAvailableSkills: () => ({ skills: [], loading: false, refresh: vi.fn() }),
}));

vi.mock('../../lib/composio/hooks', () => ({
  useComposioIntegrations: () => ({
    toolkits: composioToolkits,
    connectionByToolkit: composioConnectionByToolkit,
    refresh: composioRefresh,
    loading: false,
    error: composioError,
  }),
}));

describe('Skills page — Composio catalog fallback', () => {
  beforeEach(() => {
    composioRefresh = vi.fn();
    composioError = null;
    composioToolkits = [];
    composioConnectionByToolkit = new Map();
  });

  it('shows known composio integrations in the integrations icon grid when the live toolkit list is empty', () => {
    renderWithProviders(<Skills />, { initialEntries: ['/skills'] });

    expect(screen.getByRole('heading', { name: 'Integrations' })).toBeInTheDocument();
    expect(screen.getByText('微博')).toBeInTheDocument();
    expect(screen.getByText('小红书')).toBeInTheDocument();
    expect(screen.getByText('抖音')).toBeInTheDocument();
    expect(screen.getByText('快手')).toBeInTheDocument();
    expect(screen.getByText('B站')).toBeInTheDocument();
    expect(screen.getByText('知乎')).toBeInTheDocument();
    expect(screen.getByText('阿里云')).toBeInTheDocument();
    expect(screen.getByText('百度 AI')).toBeInTheDocument();
    expect(screen.getByText('支付宝')).toBeInTheDocument();
    const integrationsSection = screen
      .getByRole('heading', { name: 'Integrations' })
      .closest('.rounded-2xl');
    expect(integrationsSection).not.toBeNull();
    expect(within(integrationsSection as HTMLElement).getByText('码云 Gitee')).toBeInTheDocument();
  });

  it('shows disconnected tiles (not error state) when composio backend is unreachable and toolkit list is empty', () => {
    composioError = 'Backend unavailable';
    composioToolkits = [];

    renderWithProviders(<Skills />, { initialEntries: ['/skills'] });

    // Error banner should NOT appear when using fallback catalog (no real toolkits)
    expect(screen.queryByText('Connections are showing stale status')).not.toBeInTheDocument();

    const integrationsSection = screen
      .getByRole('heading', { name: 'Integrations' })
      .closest('.rounded-2xl');
    expect(integrationsSection).not.toBeNull();
    // Tiles show as disconnected with "Connect" CTA, not "Status unavailable"
    const weiboTile = within(integrationsSection as HTMLElement).getByRole('button', {
      name: /^微博,.*Connect\./i,
    });
    expect(weiboTile).toBeInTheDocument();
  });

  it('shows error state when composio fails after returning real toolkits', () => {
    composioError = 'Backend unavailable';
    composioToolkits = ['weibo'];

    renderWithProviders(<Skills />, { initialEntries: ['/skills'] });

    expect(screen.getByText('Connections are showing stale status')).toBeInTheDocument();
    expect(screen.getByText('Backend unavailable')).toBeInTheDocument();

    const integrationsSection = screen
      .getByRole('heading', { name: 'Integrations' })
      .closest('.rounded-2xl');
    expect(integrationsSection).not.toBeNull();
    const weiboTile = within(integrationsSection as HTMLElement).getByRole('button', {
      name: /^微博,.*Status unavailable/i,
    });
    expect(weiboTile).toBeInTheDocument();

    fireEvent.click(screen.getAllByRole('button', { name: 'Retry' })[0]);
    expect(composioRefresh).toHaveBeenCalledTimes(1);
  });
});
