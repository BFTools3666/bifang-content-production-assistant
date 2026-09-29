/** 在用户可见的公开页面读取作品，不读取Cookie、存储、登录态或网络私有接口。 */
(() => {
  const platform = __BIFANG_PLATFORM__;
  const session = __BIFANG_SESSION__;
  const now = Date.now();

  /** 仅解析页面实际展示的数字，未展示的指标保持null。 */
  function count(value) {
    if (value === undefined || value === null) return null;
    const text = String(value).trim().replace(/,/g, "");
    const match = text.match(/^(\d+(?:\.\d+)?)\s*(万|亿|[kKwWmM])?\+?$/);
    if (!match) return null;
    const multipliers = { 万: 10000, 亿: 100000000, k: 1000, w: 10000, m: 1000000 };
    return Math.round(Number(match[1]) * (multipliers[(match[2] || "").toLowerCase()] || 1));
  }

  /** 清理可见文本并限制长度，不解释网页中的指令。 */
  function text(element, maximum = 24000) {
    return (element?.innerText || element?.textContent || "").trim().slice(0, maximum);
  }

  /** 将页面相对公开链接转换为HTTPS绝对链接。 */
  function absolute(address) {
    if (!address || typeof address !== 'string' || !address.trim()) return '';
    try {
      const url = new URL(address, location.href);
      return url.protocol === "https:" ? url.href : "";
    } catch { return ""; }
  }

  /** 从某个公开区域读取明确标注的指标，避免把年份或标题数字当成互动量。 */
  function labeled(region, labels) {
    const content = text(region, 10000);
    for (const label of labels) {
      const after = content.match(new RegExp(`${label}\\s*[:：]?\\s*([\\d,.]+\\s*(?:万|亿|[kKwWmM])?\\+?)`));
      const before = content.match(new RegExp(`([\\d,.]+\\s*(?:万|亿|[kKwWmM])?\\+?)\\s*${label}`));
      const found = after || before;
      if (found) return count(found[1]);
    }
    return null;
  }

  /** CSDN搜索有仅显示图标与数字的布局；依据公开图标语义读取，不按位置猜指标。 */
  function csdnListMetrics(region) {
    const metrics = { views: labeled(region, ["阅读量", "阅读", "浏览"]), likes: labeled(region, ["点赞", "赞"]), favorites: labeled(region, ["收藏"]), comments: labeled(region, ["评论"]), shares: null };
    for (const icon of region?.querySelectorAll('img, svg use, i[class^="icon-"], i[class*=" icon-"]') || []) {
      const semantic = [icon.getAttribute('src'), icon.getAttribute('href'), icon.getAttribute('xlink:href'), icon.getAttribute('alt'), icon.getAttribute('title'), icon.getAttribute('class')].filter(Boolean).join(' ');
      const metric = /(?:^|[-_/#])(eye|read|view)(?:[-_. ]|$)|阅读|浏览/i.test(semantic) ? 'views'
        : /(?:^|[-_/#])(good|like|digg|dig)(?:[-_. ]|$)|点赞/i.test(semantic) ? 'likes'
        : /(?:^|[-_/#])(comment)(?:[-_. ]|$)|评论/i.test(semantic) ? 'comments'
        : /(?:^|[-_/#])(collect|favorite)(?:[-_. ]|$)|收藏/i.test(semantic) ? 'favorites' : null;
      if (!metric || metrics[metric] !== null) continue;
      const item = icon.closest('.eng-item') || (icon.tagName.toLowerCase() === 'use' ? icon.closest('svg')?.parentElement : icon.parentElement);
      metrics[metric] = count(text(item));
    }
    return metrics;
  }

  /** 创建只包含公开作品字段的结果，剔除无关页面数据。 */
  function work(title, url, extra = {}) {
    return { platform, url, authorUrl: "", metrics: { views: null, likes: null, favorites: null, comments: null, shares: null }, collectedAtMs: now, ...extra,
      title: String(title || "").trim().slice(0, 500), author: String(extra.author || "").trim().slice(0, 200), publishedAt: String(extra.publishedAt || "").slice(0, 100), excerpt: String(extra.excerpt || "").slice(0, 2000), body: String(extra.body || "").trim().slice(0, 24000) };
  }

  /** CSDN域名式文章没有路径中的用户名；主页必须取作者域名，避免生成 /article 假主页。 */
  function csdnHome(address) {
    try {
      const url = new URL(address, location.href);
      const vanity = url.hostname.match(/^([a-zA-Z0-9_-]+)\.blog\.csdn\.net$/);
      const username = vanity?.[1] || (url.hostname === 'blog.csdn.net' ? url.pathname.split('/')[1] : '');
      return username && !['article', 'rank'].includes(username) ? `https://blog.csdn.net/${username}` : '';
    } catch { return ''; }
  }

  /** 使用CSDN可见搜索卡片、主页列表或文章正文提取公开作品。 */
  function csdn() {
    const results = [];
    const detail = /\/article\/details\/\d+/.test(location.pathname);
    if (detail) {
      const content = document.querySelector("#content_views, .blog-content-box, #article_content");
      const author = document.querySelector("#uid, .follow-nickName, .blogger a, a[href*='/'].username, .profile-intro a");
      const authorLink = author?.closest("a")?.href || document.querySelector(".blogger a[href*='blog.csdn.net']")?.href;
      const username = author?.querySelector('[username]')?.getAttribute('username');
      const fallback = username ? `https://blog.csdn.net/${username}` : csdnHome(authorLink || location.href);
      const region = document.querySelector(".article-info-box") || document.querySelector(".blog-content-box") || document.body;
      const metrics = {
        views: count(text(document.querySelector(".read-count, .article-read-count"))) ?? labeled(region, ["阅读量", "阅读", "浏览"]),
        likes: count(text(document.querySelector("#blog-digg-num, #spanCount, .toolbox-bottom .count"))) ?? labeled(region, ["点赞", "赞"]),
        favorites: count(text(document.querySelector("#get-collection, #blog_detail_zk_collection .get-collection, .collect-count"))) ?? labeled(region, ["收藏"]),
        comments: count(text(document.querySelector(".tool-item-comment .count, .comment-count"))) ?? labeled(region, ["评论"]), shares: null,
      };
      results.push(work(text(document.querySelector("h1.title-article, h1")), location.href, { body: text(content), author: text(author, 200), authorUrl: absolute(fallback), publishedAt: text(document.querySelector(".time, .article-time"), 100), metrics }));
      return results;
    }
    for (const anchor of document.querySelectorAll("a[href*='blog.csdn.net'][href*='/article/details/']")) {
      const card = anchor.closest(".result-card-v170, .search-item, .so-search-wrap, .search-list-item, .article-item-box, .blog-list-box, .list-item, .search-list .item") || anchor.parentElement?.parentElement;
      const engagement = card?.querySelector('.meta-rt') || card;
      const title = text(anchor.querySelector("h3, h4, .title") || anchor, 500);
      const author = card?.querySelector("a[href*='blog.csdn.net']:not([href*='/article/details/'])");
      const url = absolute(anchor.href);
      const homeAuthor = card?.matches('.blog-list-box, .article-item-box') ? document.querySelector('.user-profile-head-name > div:first-child') : null;
      results.push(work(title, url, { author: text(author || homeAuthor, 200), authorUrl: csdnHome(author?.href || url), excerpt: text(card?.querySelector(".card-digest, .blog-list-content, .content, .summary, .description, .search-result-content"), 2000), metrics: csdnListMetrics(engagement) }));
    }
    return results;
  }

  /** 将小红书公开页面初始内容中的笔记字段映射为作品。 */
  function xhsCard(card, outer = {}) {
    const id = card.noteId || card.note_id || card.id || outer.id;
    const title = card.displayTitle || card.display_title || card.title || String(card.desc || "").split('\n')[0];
    if (!id || !title || !/^[a-zA-Z0-9]{16,40}$/.test(id)) return null;
    const user = card.user || {};
    const userId = user.userId || user.user_id || user.id;
    const info = card.interactInfo || card.interact_info || {};
    const token = outer.xsecToken || outer.xsec_token || card.xsecToken || card.xsec_token;
    const url = new URL(`https://www.xiaohongshu.com/explore/${id}`);
    if (token) { url.searchParams.set('xsec_token', token); url.searchParams.set('xsec_source', 'pc_search'); }
    return work(title, url.href, { author: user.nickname || user.nickName || "", authorUrl: userId ? `https://www.xiaohongshu.com/user/profile/${userId}` : "", body: card.desc || "", metrics: { views: count(info.viewCount ?? info.view_count), likes: count(info.likedCount ?? info.liked_count), favorites: count(info.collectedCount ?? info.collected_count), comments: count(info.commentCount ?? info.comment_count), shares: count(info.shareCount ?? info.share_count) } });
  }

  /** 只遍历公开笔记容器，不读取页面认证对象和存储。 */
  function xhs() {
    const results = [];
    const state = window.__INITIAL_STATE__;
    const seen = new WeakSet();
    let visits = 0;
    /** 从限定公开内容树寻找作品卡片，控制深度和访问次数。 */
    function visit(value, depth = 0) {
      if (!value || typeof value !== 'object' || depth > 9 || ++visits > 15000 || seen.has(value)) return;
      seen.add(value);
      const card = value.noteCard || value.note_card || value.note;
      const candidate = card ? xhsCard(card, value) : xhsCard(value);
      if (candidate) results.push(candidate);
      for (const [key, child] of Object.entries(value)) {
        if (/token|cookie|auth|account|login|password/i.test(key)) continue;
        if (child && typeof child === 'object') visit(child, depth + 1);
      }
    }
    for (const section of [state?.search, state?.note, state?.user?.notes, state?.user?.notesList]) visit(section);
    for (const card of document.querySelectorAll("section.note-item, .note-item, .feeds-container > section")) {
      // 搜索卡片前面的空 explore 链接不带访问参数，优先保留实际可见标题链接。
      const anchor = card.querySelector("a.title[href*='/search_result/']")
        || card.querySelector("a.cover[href*='/search_result/']")
        || card.querySelector("a[href*='/explore/'], a[href*='/discovery/item/']");
      if (!anchor) continue;
      const author = card.querySelector("a[href*='/user/profile/']");
      const title = text(card.querySelector(".title, .footer .title, .note-title"), 500) || anchor.getAttribute('title');
      const candidate = work(title, absolute(anchor.href), { author: text(author?.querySelector('.name') || author, 200), authorUrl: absolute(author?.href || ""), publishedAt: text(author?.querySelector('.time'), 100), metrics: { views: null, likes: count(text(card.querySelector(".like-wrapper .count, .like-count"))), favorites: null, comments: null, shares: null } });
      results.push(candidate);
    }
    if (/\/(explore|discovery\/item|search_result)\/[a-zA-Z0-9]{16,40}\/?$/.test(location.pathname)) {
      const id = location.pathname.split('/').filter(Boolean).pop();
      const existing = results.find(item => item.url.includes(`/${id}`));
      const author = document.querySelector(".author-container a[href*='/user/profile/'], a.author[href*='/user/profile/']");
      const title = text(document.querySelector("#detail-title, .note-content .title, .note-scroller .title"), 500);
      const body = text(document.querySelector("#detail-desc, .note-content .desc, .note-text"));
      const candidate = existing || work(title, location.href);
      candidate.url = location.href;
      if (title) candidate.title = title;
      if (body) candidate.body = body;
      if (author) { candidate.author = text(author, 200); candidate.authorUrl = absolute(author.href); }
      const interaction = document.querySelector(".interact-container, .interactions, .engage-bar") || document.body;
      candidate.metrics.likes = count(text(interaction.querySelector(".like-wrapper .count"))) ?? candidate.metrics.likes;
      candidate.metrics.favorites = count(text(interaction.querySelector(".collect-wrapper .count"))) ?? candidate.metrics.favorites;
      candidate.metrics.comments = count(text(interaction.querySelector(".chat-wrapper .count, .comment-wrapper .count"))) ?? candidate.metrics.comments;
      return candidate.title ? [candidate] : [];
    }
    return results;
  }

  /** 从百家号详情、公开主页或百度的百家号搜索结果提取原始作品链接。 */
  function baijiahao() {
    const results = [];
    if (location.hostname === 'baijiahao.baidu.com' && /\/s\/?$/.test(location.pathname)) {
      const content = document.querySelector("#ssr-content, .article-content, .bjh-article, .article-wrap, .mainContent");
      const author = document.querySelector("a[href*='author.baidu.com/home'], a[href*='/builder/author']");
      const title = text(document.querySelector("h1, .article-title, .title"), 500);
      results.push(work(title, location.href, { body: text(content), author: text(author, 200), authorUrl: absolute(author?.href || ""), publishedAt: text(document.querySelector(".date, .publish-time"), 100), metrics: { views: labeled(document.body, ['阅读量', '阅读', '播放']), likes: labeled(document.querySelector('.article-bottom, .interact') || document.body, ['点赞', '赞']), favorites: labeled(document.body, ['收藏']), comments: labeled(document.body, ['评论']), shares: null } }));
      return results;
    }
    for (const anchor of document.querySelectorAll("a[href*='baijiahao.baidu.com/s'], h3 a, .article-item a, .note-item a")) {
      const card = anchor.closest('.result, .c-container, .article-item, .content-item') || anchor.parentElement?.parentElement;
      const address = card?.getAttribute('mu') || card?.getAttribute('data-landurl') || anchor.getAttribute('data-landurl') || anchor.href;
      const url = absolute(address);
      if (!url.startsWith('https://baijiahao.baidu.com/s')) continue;
      const author = card?.querySelector("a[href*='author.baidu.com/home']");
      results.push(work(text(anchor, 500), url, { author: text(author, 200), authorUrl: absolute(author?.href || (location.hostname === 'author.baidu.com' ? location.href : '')), excerpt: text(card?.querySelector('.c-abstract, .summary, .desc'), 2000), metrics: { views: labeled(card, ['阅读量', '阅读', '播放']), likes: labeled(card, ['点赞', '赞']), favorites: labeled(card, ['收藏']), comments: labeled(card, ['评论']), shares: null } }));
    }
    return results;
  }

  let pendingChunks = [];
  let chunkIndex = 0;
  let captureBusy = false;

  /** 等待宿主保存成功后再发送下一批，避免长正文超出WebView导航长度。 */
  function sendChunk() {
    const callback = new URL('bifang-collect://result/');
    callback.searchParams.set('session', session);
    callback.searchParams.set('payload', JSON.stringify(pendingChunks[chunkIndex]));
    location.href = callback.href;
  }

  /** 保存确认仅推进当前采集，失败时允许用户重新采集。 */
  window.__bifangCollectionAck = saved => {
    if (saved && ++chunkIndex < pendingChunks.length) sendChunk();
    else { captureBusy = false; pendingChunks = []; }
  };

  /** 发送有界的公开作品回调，宿主拦截该专用导航并原子保存。 */
  function capture() {
    if (captureBusy) return;
    const raw = platform === 'csdn' ? csdn() : platform === 'xiaohongshu' ? xhs() : baijiahao();
    const unique = new Map();
    for (const item of raw) {
      if (!item.title || !item.url || item.title === '登录' || item.title === '注册') continue;
      const noteId = platform === 'xiaohongshu' ? new URL(item.url).pathname.match(/\/(?:explore|discovery\/item|search_result)\/([a-zA-Z0-9]{16,40})\/?$/)?.[1] : null;
      const key = noteId ? `note:${noteId}` : item.url.split('?')[0] + (platform === 'baijiahao' ? new URL(item.url).searchParams.get('id') : '');
      const old = unique.get(key);
      if (!old) unique.set(key, item);
      else {
        if (platform === 'xiaohongshu' && !new URL(old.url).searchParams.has('xsec_token') && new URL(item.url).searchParams.has('xsec_token')) old.url = item.url;
        if (item.body.length > old.body.length) old.body = item.body;
        if (!old.author && item.author) old.author = item.author;
        if (!old.authorUrl && item.authorUrl) old.authorUrl = item.authorUrl;
        for (const metric of ['views', 'likes', 'favorites', 'comments', 'shares']) {
          if (old.metrics[metric] === null && item.metrics[metric] !== null) old.metrics[metric] = item.metrics[metric];
        }
      }
    }
    const works = [...unique.values()].slice(0, 100);
    const message = works.length ? `读取到${works.length}件作品` : '当前页未读取到公开作品。请完成平台登录或验证码、等待加载后点击采集当前页；搜索列表只提供公开展示的指标。';
    const batches = [];
    let batch = [];
    for (const item of works) {
      if (batch.length && encodeURIComponent(JSON.stringify([...batch, item])).length > 300000) { batches.push(batch); batch = []; }
      batch.push(item);
    }
    if (batch.length || !batches.length) batches.push(batch);
    pendingChunks = batches.map((items, index) => ({works:items, message, pageUrl:location.href, chunkIndex:index, chunkCount:batches.length}));
    chunkIndex = 0;
    captureBusy = true;
    sendChunk();
    const status = document.querySelector('#bifang-public-collector')?.shadowRoot?.querySelector('[role=status]');
    if (status) status.textContent = message;
  }

  /** 提供公开页面采集按钮；用户可直接浏览、登录、滚动和打开详情。 */
  function install() {
    if (document.querySelector('#bifang-public-collector')) return;
    const host = document.createElement('aside');
    host.id = 'bifang-public-collector';
    host.style.cssText = 'position:fixed;right:18px;bottom:18px;z-index:2147483647;max-width:340px';
    const root = host.attachShadow({mode:'open'});
    root.innerHTML = '<style>:host{font:14px/1.5 system-ui;color:#202124}section{background:#fff;border:1px solid #bbb;border-top:4px solid #12766b;padding:16px;box-shadow:0 4px 24px #0002}button{display:block;width:100%;background:#12766b;color:#fff;border:0;padding:12px;font:inherit;cursor:pointer;margin-top:10px}small{display:block;color:#555;margin-top:6px}p{margin:0}button:focus-visible{outline:3px solid #333;outline-offset:2px}</style><section><strong>毕方 · 公开作品采集</strong><p role="status">正在读取当前页面</p><small>打开作品详情可补充正文和收藏等数据；主页可读取已加载的作品。平台未公开的指标不推算。</small><button type="button">采集当前页</button></section>';
    root.querySelector('button').addEventListener('click', capture);
    document.body.append(host);
    window.setTimeout(capture, 2200);
  }
  if (document.body) install(); else document.addEventListener('DOMContentLoaded', install, {once:true});
})();
