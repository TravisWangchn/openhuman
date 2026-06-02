import { DISCORD_INVITE_URL } from '../../utils/links';
import { openUrl } from '../../utils/openUrl';

export function DiscordBanner() {
  return (
    <button
      type="button"
      onClick={() => {
        void openUrl(DISCORD_INVITE_URL);
      }}
      className="mb-3 text-left mt-3 block rounded-2xl border border-[#CDD2FF] bg-gradient-to-r from-[#F6F7FF] via-[#F1F3FF] to-[#ECEFFF] px-4 py-4 text-[#414AAE] shadow-soft transition-transform transition-colors hover:-translate-y-0.5 hover:border-[#BCC3FF] hover:from-[#EEF0FF] hover:to-[#E5E9FF]">
      <div className="flex items-center gap-3">
        <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-full bg-[#5865F2]/12 text-[#5865F2]">
          <svg className="h-5 w-5" fill="currentColor" viewBox="0 0 24 24" aria-hidden="true">
            <path d="M20.317 4.37A19.79 19.79 0 0 0 15.885 3c-.191.328-.403.775-.552 1.124a18.27 18.27 0 0 0-5.29 0A11.56 11.56 0 0 0 9.49 3a19.74 19.74 0 0 0-4.433 1.37C2.253 8.51 1.492 12.55 1.872 16.533a19.9 19.9 0 0 0 5.239 2.673c.423-.58.8-1.196 1.123-1.845a12.84 12.84 0 0 1-1.767-.85c.148-.106.292-.217.43-.332c3.408 1.6 7.104 1.6 10.472 0c.14.115.283.226.43.332c-.565.338-1.157.623-1.771.851c.322.648.698 1.264 1.123 1.844a19.84 19.84 0 0 0 5.241-2.673c.446-4.617-.761-8.621-3.787-12.164ZM9.46 14.088c-1.02 0-1.855-.936-1.855-2.084c0-1.148.82-2.084 1.855-2.084c1.044 0 1.87.944 1.855 2.084c0 1.148-.82 2.084-1.855 2.084Zm5.08 0c-1.02 0-1.855-.936-1.855-2.084c0-1.148.82-2.084 1.855-2.084c1.044 0 1.87.944 1.855 2.084c0 1.148-.812 2.084-1.855 2.084Z" />
          </svg>
        </div>
        <div className="min-w-0 flex-1">
          <div className="text-sm font-semibold">Join Our Discord</div>
          <div className="mt-0.5 text-sm text-[#5E66BC]">
            Get updates, free merch, credits, report bugs, and be part of the OpenHuman community.
          </div>
        </div>
      </div>
    </button>
  );
}
