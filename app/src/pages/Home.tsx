import { useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';

import ConnectionIndicator from '../components/ConnectionIndicator';
import { DiscordBanner } from '../components/home/HomeBanners';
import { useUser } from '../hooks/useUser';
import { useT } from '../lib/i18n/I18nContext';
import { restartCoreProcess } from '../services/coreProcessControl';
import { selectBlockingState } from '../store/connectivitySelectors';
import { useAppSelector } from '../store/hooks';
import { APP_VERSION } from '../utils/config';

export function resolveHomeUserName(user: unknown): string {
  if (!user || typeof user !== 'object') return 'User';

  const record = user as Record<string, unknown>;
  const firstName =
    (typeof record.firstName === 'string' && record.firstName.trim()) ||
    (typeof record.first_name === 'string' && record.first_name.trim()) ||
    '';
  const lastName =
    (typeof record.lastName === 'string' && record.lastName.trim()) ||
    (typeof record.last_name === 'string' && record.last_name.trim()) ||
    '';
  const username = typeof record.username === 'string' ? record.username.trim() : '';
  const email = typeof record.email === 'string' ? record.email.trim() : '';

  const fullName = [firstName, lastName].filter(Boolean).join(' ').trim();
  if (fullName) return fullName;
  if (firstName) return firstName;
  if (username) return username.startsWith('@') ? username : `@${username}`;
  if (email) return email.split('@')[0] || 'User';
  return 'User';
}

const Home = () => {
  const { t } = useT();
  const { user } = useUser();
  const navigate = useNavigate();
  const _userName = resolveHomeUserName(user);
  const userName = _userName.split(' ')[0];

  const welcomeVariants = useMemo(
    () => [`Welcome, ${userName} 👋`, `Let's cook, ${userName} 🧑‍🍳.`, `Time to Zone In 🧘🏻`],
    [userName]
  );
  const [welcomeVariantIndex, setWelcomeVariantIndex] = useState(0);
  const [typedWelcome, setTypedWelcome] = useState('');
  const [isDeletingWelcome, setIsDeletingWelcome] = useState(false);
  const blocking = useAppSelector(selectBlockingState);
  const [isRestartingCore, setIsRestartingCore] = useState(false);
  const [restartError, setRestartError] = useState<string | null>(null);

  const handleRestartCore = async () => {
    setIsRestartingCore(true);
    setRestartError(null);
    try {
      await restartCoreProcess();
    } catch (err) {
      setRestartError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsRestartingCore(false);
    }
  };

  const statusCopy = {
    ok: t('home.statusOk'),
    'backend-only': t('home.statusBackendOnly'),
    'core-unreachable': t('home.statusCoreUnreachable'),
    'internet-offline': t('home.statusInternetOffline'),
  }[blocking];

  const handleStartCooking = async () => {
    navigate('/chat');
  };

  useEffect(() => {
    const activeVariant = welcomeVariants[welcomeVariantIndex] ?? '';
    const isFullyTyped = typedWelcome === activeVariant;
    const isFullyDeleted = typedWelcome.length === 0;

    const delay = isDeletingWelcome
      ? 36
      : isFullyTyped
        ? 1400
        : typedWelcome.length === 0
          ? 250
          : 55;

    const timeoutId = window.setTimeout(() => {
      if (!isDeletingWelcome) {
        if (isFullyTyped) {
          setIsDeletingWelcome(true);
          return;
        }

        setTypedWelcome(activeVariant.slice(0, typedWelcome.length + 1));
        return;
      }

      if (!isFullyDeleted) {
        setTypedWelcome(activeVariant.slice(0, typedWelcome.length - 1));
        return;
      }

      setIsDeletingWelcome(false);
      setWelcomeVariantIndex(current => (current + 1) % welcomeVariants.length);
    }, delay);

    return () => window.clearTimeout(timeoutId);
  }, [isDeletingWelcome, typedWelcome, welcomeVariantIndex, welcomeVariants]);

  return (
    <div className="min-h-full flex flex-col items-center justify-center p-4">
      <div className="max-w-md w-full">
        {/* Main card — data-walkthrough target for step 1 */}
        <div
          data-walkthrough="home-card"
          className="bg-white rounded-2xl shadow-soft border border-stone-200 p-6 animate-fade-up">
          {/* Header row: logo + version + settings */}
          <div className="flex items-center justify-center mb-4">
            <span className="text-xs text-center text-stone-400">v{APP_VERSION}</span>
          </div>

          {/* Welcome title */}
          <h1 className="min-h-[3.5rem] text-32l font-bold text-stone-900 text-center">
            {typedWelcome}
            <span aria-hidden="true" className="ml-0.5 inline-block text-primary-500 animate-pulse">
              |
            </span>
          </h1>

          {/* Connection status */}
          <div className="flex justify-center mb-3">
            <ConnectionIndicator />
          </div>

          <p className="text-sm text-stone-500 text-center mb-6 leading-relaxed">{statusCopy}</p>

          {blocking === 'core-unreachable' && (
            <div className="mb-4">
              <button
                onClick={handleRestartCore}
                disabled={isRestartingCore}
                className="w-full py-3 bg-amber-500 hover:bg-amber-600 disabled:opacity-50 text-white font-medium rounded-xl transition-colors duration-200">
                {isRestartingCore ? t('home.restartingCore') : t('home.restartCore')}
              </button>
              {restartError && (
                <p className="mt-2 text-xs text-coral-500 text-center">{restartError}</p>
              )}
            </div>
          )}

          {/* CTA button — data-walkthrough target for step 2 */}
          <button
            data-walkthrough="home-cta"
            onClick={handleStartCooking}
            disabled={blocking === 'core-unreachable' || blocking === 'internet-offline'}
            className="w-full py-3 bg-primary-500 hover:bg-primary-600 disabled:opacity-50 disabled:cursor-not-allowed text-white font-medium rounded-xl transition-colors duration-200">
            {t('home.askAssistant')}
          </button>
        </div>

        <DiscordBanner />
      </div>
    </div>
  );
};

export default Home;
