import { useCallback, useEffect, useState } from 'react';

import type { PlanTier } from '../types/api';
import { subscribeUsageRefresh } from './usageRefresh';

// Stub type retained for downstream consumers that still reference teamUsage
// fields.  Since cloud billing is removed, teamUsage is always null.
export interface TeamUsage {
  remainingUsd: number;
  cycleBudgetUsd: number;
  cycleLimit5hr: number;
  cycleLimit7day: number;
  fiveHourCapUsd: number;
  fiveHourResetsAt: string | null;
  cycleStartDate: string;
  cycleEndsAt: string;
  bypassCycleLimit?: boolean;
}

export interface UsageState {
  teamUsage: TeamUsage | null;
  currentPlan: null;
  currentTier: PlanTier;
  isFreeTier: boolean;
  usagePct10h: number;
  usagePct7d: number;
  isNearLimit: boolean;
  isAtLimit: boolean;
  isRateLimited: boolean;
  isBudgetExhausted: boolean;
  shouldShowBudgetCompletedMessage: boolean;
  isLoading: boolean;
  refresh: () => void;
}

export function useUsageState(): UsageState {
  const [, setFetchCount] = useState(0);

  const refresh = useCallback(() => {
    setFetchCount(c => c + 1);
  }, []);

  useEffect(() => subscribeUsageRefresh(refresh), [refresh]);

  // Cloud billing removed — all budget/limit flags are hard-disabled.
  // DeepSeek API key is managed at https://platform.deepseek.com/top_up
  return {
    teamUsage: null,
    currentPlan: null,
    currentTier: 'FREE' as PlanTier,
    isFreeTier: true,
    usagePct10h: 0,
    usagePct7d: 0,
    isNearLimit: false,
    isAtLimit: false,
    isRateLimited: false,
    isBudgetExhausted: false,
    shouldShowBudgetCompletedMessage: false,
    isLoading: false,
    refresh,
  };
}
