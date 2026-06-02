import type { ChannelConnectionStatus, ChannelDefinition } from '../../types/channels';

/** Status badge styles for channel connection states. */
export const STATUS_STYLES: Record<ChannelConnectionStatus, { label: string; className: string }> =
  {
    connected: { label: 'Connected', className: 'bg-sage-500/10 text-sage-700 border-sage-500/30' },
    connecting: {
      label: 'Connecting',
      className: 'bg-amber-500/10 text-amber-700 border-amber-500/30',
    },
    disconnected: {
      label: 'Disconnected',
      className: 'bg-stone-100 text-stone-500 border-stone-200',
    },
    error: { label: 'Error', className: 'bg-coral-500/10 text-coral-700 border-coral-500/30' },
  };

/** Human-readable labels for auth modes. */
export const AUTH_MODE_LABELS: Record<string, string> = {
  managed_dm: 'Login with OpenHuman',
  oauth: 'OAuth Sign-in',
  bot_token: 'Use your own Bot Token',
  api_key: 'Use your own API Key',
};

/** Fallback definitions used when the core sidecar is unreachable. */
export const FALLBACK_DEFINITIONS: ChannelDefinition[] = [
  {
    id: 'telegram',
    display_name: 'Telegram',
    description: 'Send and receive messages via Telegram.',
    icon: 'telegram',
    auth_modes: [
      {
        mode: 'managed_dm',
        description: 'Message the OpenHuman Telegram bot directly.',
        fields: [],
        auth_action: 'telegram_managed_dm',
      },
      {
        mode: 'bot_token',
        description: 'Provide your own Telegram Bot token from @BotFather.',
        fields: [
          {
            key: 'bot_token',
            label: 'Bot Token',
            field_type: 'secret',
            required: true,
            placeholder: '123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11',
          },
          {
            key: 'allowed_users',
            label: 'Allowed Users',
            field_type: 'string',
            required: false,
            placeholder: 'Comma-separated Telegram usernames',
          },
        ],
        auth_action: undefined,
      },
    ],
    capabilities: ['send_text', 'receive_text', 'typing', 'draft_updates'],
  },
  {
    id: 'discord',
    display_name: 'Discord',
    description: 'Send and receive messages via Discord.',
    icon: 'discord',
    auth_modes: [
      {
        mode: 'bot_token',
        description: 'Provide your own Discord bot token.',
        fields: [
          {
            key: 'bot_token',
            label: 'Bot Token',
            field_type: 'secret',
            required: true,
            placeholder: 'Your Discord bot token',
          },
          {
            key: 'guild_id',
            label: 'Server (Guild) ID',
            field_type: 'string',
            required: false,
            placeholder: 'Optional: restrict to a specific server',
          },
        ],
        auth_action: undefined,
      },
      {
        mode: 'oauth',
        description: 'Install the OpenHuman bot to your Discord server via OAuth.',
        fields: [],
        auth_action: 'discord_oauth',
      },
      {
        mode: 'managed_dm',
        description: 'Link your personal Discord account to the OpenHuman bot.',
        fields: [],
        auth_action: 'discord_managed_link',
      },
    ],
    capabilities: ['send_text', 'receive_text', 'typing', 'threaded_replies'],
  },
  {
    id: 'web',
    display_name: 'Web',
    description: 'Chat via the built-in web UI.',
    icon: 'web',
    auth_modes: [
      {
        mode: 'managed_dm',
        description: 'Use the embedded web chat — no setup required.',
        fields: [],
        auth_action: undefined,
      },
    ],
    capabilities: ['send_text', 'send_rich_text', 'receive_text'],
  },
  {
    id: 'wechat',
    display_name: '微信',
    description: '通过微信公众号或企业微信收发消息。',
    icon: 'wechat',
    auth_modes: [
      {
        mode: 'api_key',
        description: '使用微信公众号 AppID 和 AppSecret 连接。',
        fields: [
          {
            key: 'app_id',
            label: 'AppID',
            field_type: 'string',
            required: true,
            placeholder: '微信公众号 AppID',
          },
          {
            key: 'app_secret',
            label: 'AppSecret',
            field_type: 'secret',
            required: true,
            placeholder: '微信公众号 AppSecret',
          },
        ],
        auth_action: undefined,
      },
    ],
    capabilities: ['send_text', 'receive_text', 'file_attachments'],
  },
  {
    id: 'dingtalk',
    display_name: '钉钉',
    description: '通过钉钉机器人或应用收发消息。',
    icon: 'dingtalk',
    auth_modes: [
      {
        mode: 'api_key',
        description: '使用钉钉机器人 Webhook 地址连接。',
        fields: [
          {
            key: 'webhook_url',
            label: 'Webhook 地址',
            field_type: 'string',
            required: true,
            placeholder: 'https://oapi.dingtalk.com/robot/send?access_token=...',
          },
          {
            key: 'secret',
            label: '加签密钥 (可选)',
            field_type: 'secret',
            required: false,
            placeholder: '机器人安全设置中的加签密钥',
          },
        ],
        auth_action: undefined,
      },
    ],
    capabilities: ['send_text', 'receive_text', 'typing'],
  },
  {
    id: 'feishu',
    display_name: '飞书',
    description: '通过飞书机器人或应用收发消息。',
    icon: 'feishu',
    auth_modes: [
      {
        mode: 'api_key',
        description: '使用飞书应用凭证连接。',
        fields: [
          {
            key: 'app_id',
            label: 'App ID',
            field_type: 'string',
            required: true,
            placeholder: '飞书应用 App ID',
          },
          {
            key: 'app_secret',
            label: 'App Secret',
            field_type: 'secret',
            required: true,
            placeholder: '飞书应用 App Secret',
          },
        ],
        auth_action: undefined,
      },
    ],
    capabilities: ['send_text', 'receive_text', 'threaded_replies', 'file_attachments'],
  },
  {
    id: 'qq',
    display_name: 'QQ',
    description: '通过 QQ 机器人收发消息。',
    icon: 'qq',
    auth_modes: [
      {
        mode: 'bot_token',
        description: '使用 QQ 机器人 Token 连接。',
        fields: [
          {
            key: 'bot_token',
            label: 'Bot Token',
            field_type: 'secret',
            required: true,
            placeholder: 'QQ 机器人 Token',
          },
          {
            key: 'bot_id',
            label: 'Bot ID (AppID)',
            field_type: 'string',
            required: true,
            placeholder: 'QQ 机器人 AppID',
          },
        ],
        auth_action: undefined,
      },
    ],
    capabilities: ['send_text', 'receive_text'],
  },
  {
    id: 'wecom',
    display_name: '企业微信',
    description: '通过企业微信应用或机器人收发消息。',
    icon: 'wecom',
    auth_modes: [
      {
        mode: 'api_key',
        description: '使用企业微信 CorpID 和 Secret 连接。',
        fields: [
          {
            key: 'corp_id',
            label: 'CorpID (企业ID)',
            field_type: 'string',
            required: true,
            placeholder: '企业微信 CorpID',
          },
          {
            key: 'corp_secret',
            label: 'CorpSecret (应用Secret)',
            field_type: 'secret',
            required: true,
            placeholder: '企业微信应用 Secret',
          },
          {
            key: 'agent_id',
            label: 'AgentID (应用ID)',
            field_type: 'string',
            required: true,
            placeholder: '企业微信应用 AgentID',
          },
        ],
        auth_action: undefined,
      },
    ],
    capabilities: ['send_text', 'receive_text', 'file_attachments', 'typing'],
  },
];
