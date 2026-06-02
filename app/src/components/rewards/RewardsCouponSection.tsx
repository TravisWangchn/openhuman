import { DEEPSEEK_TOPUP_URL } from '../../utils/links';
import { openUrl } from '../../utils/openUrl';

const RewardsCouponSection = () => {
  return (
    <div className="rounded-2xl border border-stone-200 bg-white p-4 text-left">
      <h3 className="text-sm font-semibold text-stone-900 mb-2">API Credits</h3>
      <p className="text-xs text-stone-500 mb-3">
        This app uses the DeepSeek API. Top up your credits at the DeepSeek Platform.
      </p>
      <button
        type="button"
        onClick={() => {
          void openUrl(DEEPSEEK_TOPUP_URL);
        }}
        className="w-full py-2 rounded-xl bg-primary-600 hover:bg-primary-500 text-white text-sm font-medium transition-colors">
        Top Up on DeepSeek
      </button>
    </div>
  );
};

export default RewardsCouponSection;
