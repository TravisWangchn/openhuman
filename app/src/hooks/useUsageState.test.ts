import { renderHook } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

describe('useUsageState', () => {
  it('returns all flags as disabled (cloud billing removed)', async () => {
    const { useUsageState } = await import('./useUsageState');
    const { result } = renderHook(() => useUsageState());

    expect(result.current.teamUsage).toBeNull();
    expect(result.current.currentPlan).toBeNull();
    expect(result.current.currentTier).toBe('FREE');
    expect(result.current.isFreeTier).toBe(true);
    expect(result.current.usagePct10h).toBe(0);
    expect(result.current.usagePct7d).toBe(0);
    expect(result.current.isNearLimit).toBe(false);
    expect(result.current.isAtLimit).toBe(false);
    expect(result.current.isRateLimited).toBe(false);
    expect(result.current.isBudgetExhausted).toBe(false);
    expect(result.current.shouldShowBudgetCompletedMessage).toBe(false);
    expect(result.current.isLoading).toBe(false);
  });

  it('returns a working refresh function', async () => {
    const { useUsageState } = await import('./useUsageState');
    const { result } = renderHook(() => useUsageState());

    expect(typeof result.current.refresh).toBe('function');
    // refresh should not throw
    expect(() => result.current.refresh()).not.toThrow();
  });
});
