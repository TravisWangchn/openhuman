import { describe, expect, it } from 'vitest';

import { composioToolkitMeta, KNOWN_COMPOSIO_TOOLKITS } from './toolkitMeta';

describe('composioToolkitMeta', () => {
  it('ships the full Composio managed-auth catalog fallback', () => {
    expect(KNOWN_COMPOSIO_TOOLKITS).toHaveLength(34);
    expect(KNOWN_COMPOSIO_TOOLKITS).toContain('weibo');
    expect(KNOWN_COMPOSIO_TOOLKITS).toContain('douyin');
    expect(KNOWN_COMPOSIO_TOOLKITS).toContain('alipay');
    expect(KNOWN_COMPOSIO_TOOLKITS).toContain('gitee');
  });

  it('preserves canonical names for managed-auth toolkits and renders logo URLs', () => {
    const douyin = composioToolkitMeta('douyin');
    const alipay = composioToolkitMeta('alipay');

    expect(douyin.name).toBe('抖音');
    expect(douyin.logoUrl).toContain('/douyin');
    expect(douyin.permissionLabel).toBe('内容、个人资料和社交数据');

    expect(alipay.slug).toBe('alipay');
    expect(alipay.name).toBe('支付宝');
    expect(alipay.logoUrl).toContain('/alipay');
  });

  it('falls back cleanly for unknown slugs', () => {
    const meta = composioToolkitMeta('my_custom_toolkit');

    expect(meta.slug).toBe('my_custom_toolkit');
    expect(meta.name).toBe('My Custom Toolkit');
    expect(meta.logoUrl).toContain('/my_custom_toolkit');
  });
});
