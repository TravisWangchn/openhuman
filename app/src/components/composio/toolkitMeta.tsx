/**
 * Display metadata for integration toolkits shown in the Skills grid.
 *
 * China-localized catalog replacing the foreign Composio managed-auth
 * toolkits. Provides stable names, categories, descriptions, and logos
 * for Chinese domestic services (WeChat, DingTalk, Feishu, Alibaba Cloud,
 * Baidu AI, etc.). The live toolkit list from the backend still wins for
 * runtime availability; this file provides the frontend fallback catalog.
 */
import { type ReactNode, useState } from 'react';

import { canonicalizeComposioToolkitSlug } from '../../lib/composio/toolkitSlug';
import type { SkillCategory } from '../skills/skillCategories';

export interface ComposioToolkitMeta {
  /** Toolkit slug as returned by the backend, e.g. `"gmail"`. */
  slug: string;
  /** Display name shown on the card, e.g. `"Gmail"`. */
  name: string;
  /** Short description shown on the card. */
  description: string;
  /** Which Skills page category to group the card under. */
  category: SkillCategory;
  /** Small branded icon rendered on the card and connect modal. */
  icon: ReactNode;
  /** Composio-hosted logo URL for richer provider branding. */
  logoUrl: string;
  /** Short UX hint for what the user is authorizing. */
  permissionLabel: string;
}

interface ManagedToolkitEntry {
  slug: string;
  name: string;
}

const MANAGED_COMPOSIO_TOOLKITS: readonly ManagedToolkitEntry[] = Object.freeze([
  // ── 社交 / Social ──
  { slug: 'weibo', name: '微博' },
  { slug: 'xiaohongshu', name: '小红书' },
  { slug: 'douyin', name: '抖音' },
  { slug: 'kuaishou', name: '快手' },
  { slug: 'bilibili', name: 'B站' },
  { slug: 'zhihu', name: '知乎' },
  // ── 办公协作 / Productivity ──
  { slug: 'wps', name: 'WPS 金山文档' },
  { slug: 'tencent_docs', name: '腾讯文档' },
  { slug: 'feishu_docs', name: '飞书文档' },
  { slug: 'shimo', name: '石墨文档' },
  { slug: 'teambition', name: 'Teambition' },
  { slug: 'yuque', name: '语雀' },
  // ── 云平台 / Platform ──
  { slug: 'aliyun', name: '阿里云' },
  { slug: 'tencent_cloud', name: '腾讯云' },
  { slug: 'baidu_cloud', name: '百度云' },
  { slug: 'volcengine', name: '火山引擎' },
  { slug: 'huawei_cloud', name: '华为云' },
  { slug: 'jd_cloud', name: '京东云' },
  { slug: 'gitee', name: '码云 Gitee' },
  { slug: 'coding', name: 'Coding.net' },
  // ── AI 平台 / AI ──
  { slug: 'baidu_ai', name: '百度 AI' },
  { slug: 'wenxin', name: '文心一言' },
  { slug: 'tongyi_qianwen', name: '通义千问' },
  { slug: 'iflytek_spark', name: '讯飞星火' },
  { slug: 'zhipu_ai', name: '智谱 AI' },
  { slug: 'moonshot_ai', name: '月之暗面' },
  { slug: 'deepseek', name: 'DeepSeek' },
  { slug: 'doubao', name: '豆包' },
  // ── 设计工具 / Tools ──
  { slug: 'lanhu', name: '蓝湖' },
  { slug: 'jishisheji', name: '即时设计' },
  { slug: 'processon', name: 'ProcessOn' },
  { slug: 'mokup', name: '墨刀' },
  // ── 支付 / Payments ──
  { slug: 'alipay', name: '支付宝' },
  { slug: 'wechat_pay', name: '微信支付' },
]);

const MANAGED_TOOLKIT_NAME_BY_SLUG = new Map(
  MANAGED_COMPOSIO_TOOLKITS.map(entry => [entry.slug, entry.name])
);

const CHAT_KEYWORDS = ['wechat', 'dingtalk', 'feishu', 'qq', 'wecom'];
const SOCIAL_KEYWORDS = ['weibo', 'xiaohongshu', 'douyin', 'kuaishou', 'bilibili', 'zhihu'];
const PRODUCTIVITY_KEYWORDS = [
  'wps',
  'docs',
  'shimo',
  'teambition',
  'yuque',
  'lanhu',
  'jishisheji',
  'processon',
  'mokup',
];
const PLATFORM_KEYWORDS = [
  'aliyun',
  'cloud',
  'gitee',
  'coding',
  'volcengine',
  'baidu_ai',
  'wenxin',
  'tongyi',
  'iflytek',
  'zhipu',
  'moonshot',
  'deepseek',
  'doubao',
  'alipay',
  'wechat_pay',
];

function GenericIntegrationIcon() {
  return (
    <span className="flex h-8 w-8 items-center justify-center rounded-xl bg-stone-100 text-stone-600 shadow-sm ring-1 ring-black/5">
      <svg className="h-[18px] w-[18px]" viewBox="0 0 24 24" aria-hidden="true" fill="none">
        <path
          d="M8 8h8v8H8zM5 12h3m8 0h3M12 5v3m0 8v3"
          stroke="currentColor"
          strokeWidth="1.7"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </svg>
    </span>
  );
}

function ComposioLogoBadge({ slug, name }: { slug: string; name: string }) {
  const [failed, setFailed] = useState(false);
  const logoUrl = composioLogoUrl(slug);

  if (failed) {
    return <GenericIntegrationIcon />;
  }

  return (
    <span className="flex h-8 w-8 items-center justify-center overflow-hidden rounded-xl bg-white shadow-sm ring-1 ring-black/5">
      <img
        src={logoUrl}
        alt={`${name} logo`}
        className="h-full w-full object-contain p-1"
        loading="lazy"
        onError={() => setFailed(true)}
      />
    </span>
  );
}

function composioLogoUrl(slug: string): string {
  return `https://logos.composio.dev/api/${slug}`;
}

function guessCategory(slug: string, name: string): SkillCategory {
  const key = `${slug} ${name}`.toLowerCase();
  if (CHAT_KEYWORDS.some(keyword => key.includes(keyword))) return 'Chat';
  if (SOCIAL_KEYWORDS.some(keyword => key.includes(keyword))) return 'Social';
  if (PRODUCTIVITY_KEYWORDS.some(keyword => key.includes(keyword))) return 'Productivity';
  if (PLATFORM_KEYWORDS.some(keyword => key.includes(keyword))) return 'Platform';
  return 'Tools & Automation';
}

function defaultDescription(name: string, category: SkillCategory): string {
  switch (category) {
    case 'Chat':
      return `连接${name}，实现消息收发和团队沟通。`;
    case 'Social':
      return `连接${name}，实现内容发布和社区管理。`;
    case 'Productivity':
      return `连接${name}，实现文档协作和日常办公。`;
    case 'Platform':
      return `连接${name}，实现开发、运维和平台管理。`;
    default:
      return `连接${name}。`;
  }
}

function permissionLabelFor(category: SkillCategory): string {
  switch (category) {
    case 'Chat':
      return '消息、频道和通讯数据';
    case 'Social':
      return '内容、个人资料和社交数据';
    case 'Productivity':
      return '文档、文件和协作数据';
    case 'Platform':
      return '仓库、系统和管理数据';
    default:
      return '已连接账户数据';
  }
}

function prettifyUnknownSlug(slug: string): string {
  return slug
    .split(/[_-]+/)
    .filter(Boolean)
    .map(part => part.charAt(0).toUpperCase() + part.slice(1))
    .join(' ');
}

/**
 * Canonical toolkit slugs used as the default catalog when the backend
 * allowlist hasn't loaded yet. One entry per Composio managed-auth
 * integration.
 */
export const KNOWN_COMPOSIO_TOOLKITS = Object.freeze(
  MANAGED_COMPOSIO_TOOLKITS.map(entry => entry.slug)
);

export function composioToolkitMeta(slug: string): ComposioToolkitMeta {
  const key = canonicalizeComposioToolkitSlug(slug);
  const name = MANAGED_TOOLKIT_NAME_BY_SLUG.get(key) ?? prettifyUnknownSlug(key);
  const category = guessCategory(key, name);
  return {
    slug: key,
    name,
    description: defaultDescription(name, category),
    category,
    icon: <ComposioLogoBadge slug={key} name={name} />,
    logoUrl: composioLogoUrl(key),
    permissionLabel: permissionLabelFor(category),
  };
}
