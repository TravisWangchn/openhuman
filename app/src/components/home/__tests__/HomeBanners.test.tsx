import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { DISCORD_INVITE_URL } from '../../../utils/links';
import { openUrl } from '../../../utils/openUrl';
import { DiscordBanner } from '../HomeBanners';

vi.mock('../../../utils/openUrl', () => ({ openUrl: vi.fn() }));

describe('HomeBanners', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('opens the Discord invite through openUrl from the Discord banner', () => {
    render(<DiscordBanner />);

    fireEvent.click(screen.getByRole('button', { name: /join our discord/i }));

    expect(openUrl).toHaveBeenCalledWith(DISCORD_INVITE_URL);
  });
});
