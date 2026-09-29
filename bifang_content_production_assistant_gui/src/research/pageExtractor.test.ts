import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { BenchmarkWork } from "./researchTypes";

/** 执行真实提取脚本；仅将导航目的地保留在测试对象中。 */
function capture(platform: string, address: string): { works: BenchmarkWork[]; chunkCount: number; next: () => BenchmarkWork[] } {
  const url = new URL(address);
  const location = { href: url.href, pathname: url.pathname, hostname: url.hostname };
  const source = readFileSync(resolve(process.cwd(), "src-tauri/src/research/page_extractor.js"), "utf8")
    .replace("__BIFANG_PLATFORM__", JSON.stringify(platform))
    .replace("__BIFANG_SESSION__", JSON.stringify("test-session"));
  new Function("location", source)(location);
  vi.advanceTimersByTime(2200);
  const callback = new URL(location.href);
  expect(callback.protocol).toBe("bifang-collect:");
  return {...JSON.parse(callback.searchParams.get("payload")!), next: () => {
    Reflect.get(window, "__bifangCollectionAck")(true);
    return JSON.parse(new URL(location.href).searchParams.get("payload")!).works;
  }};
}

beforeEach(() => { document.body.innerHTML = ""; vi.useFakeTimers(); Reflect.deleteProperty(window, "__INITIAL_STATE__"); });
afterEach(() => { vi.useRealTimers(); Reflect.deleteProperty(window, "__bifangCollectionAck"); });

describe("公开页面提取", () => {
  it("支持当前CSDN搜索卡片，并避免把标题中的数字当作互动数", () => {
    document.body.innerHTML = '<div class="result-card-v170"><div class="card-main"><h2><a href="https://blog.csdn.net/example/article/details/123456">收藏 99 个脚本</a></h2><p class="card-digest">公开摘要</p></div><div class="meta-row-v170"><a href="https://blog.csdn.net/example">公开作者</a><div class="meta-rt">浏览 656 点赞 8 评论 0</div></div></div>';
    const result = capture("csdn", "https://so.csdn.net/so/search?q=Python");
    expect(result.works[0]?.metrics).toEqual({views:656,likes:8,favorites:null,comments:0,shares:null});
    expect(result.works[0]?.author).toBe("公开作者");
    expect(result.works[0]?.excerpt).toBe("公开摘要");
  });

  it("区分缺失指标与零，并保留正文和主页", () => {
    document.body.innerHTML = '<h1 class="title-article">Python 自动化教程</h1><div id="content_views">公开教程正文</div><div class="article-info-box"><span class="read-count">1.2万</span></div><span id="get-collection">0</span>';
    const result = capture("csdn", "https://blog.csdn.net/example/article/details/123456");
    expect(result.works).toHaveLength(1);
    expect(result.works[0]?.metrics).toEqual({views:12000,likes:null,favorites:0,comments:null,shares:null});
    expect(result.works[0]?.body).toBe("公开教程正文");
    expect(result.works[0]?.authorUrl).toBe("https://blog.csdn.net/example");
  });

  it("识别CSDN仅图标与数字的搜索指标，未知图标不猜测", () => {
    document.body.innerHTML = '<div class="result-card-v170"><h2><a href="https://blog.csdn.net/example/article/details/123456">收藏 99 个脚本</a></h2><div class="meta-rt"><span class="eng-item"><img src="https://csdnimg.cn/eye-line-16.svg"><span>17万+</span></span><span class="eng-item"><img src="https://csdnimg.cn/good-line-16.svg"><span>854</span></span><span class="eng-item"><svg><use href="#icon-comment-line"></use></svg><span>0</span></span><span><img src="https://csdnimg.cn/unknown.svg">99</span></div></div>';
    const result = capture("csdn", "https://so.csdn.net/so/search?q=Python");
    expect(result.works[0]?.metrics).toEqual({views:170000,likes:854,favorites:null,comments:0,shares:null});
  });

  it("兼容公开搜索前端仍使用的 CSS 图标布局，不读取日期或未知图标", () => {
    document.body.innerHTML = '<div class="list-item"><h3><a href="https://blog.csdn.net/example/article/details/123456">Python教程2026</a></h3><div class="bottom"><div class="btm-lt"><div class="btm-view"><i class="icon-view"></i><span class="num">17万+</span></div><div class="btm-dig"><i class="icon-dig"></i><span class="num">854</span></div><div class="btm-comment"><i class="icon-comment"></i><span class="num">25</span></div><div><i class="icon-level"></i><span>99</span></div></div><span class="user-time">2026-08-15</span></div></div>';
    const result = capture("csdn", "https://so.csdn.net/so/search?q=Python");
    expect(result.works[0]?.metrics).toEqual({views:170000,likes:854,favorites:null,comments:25,shares:null});
  });

  it("域名式文章归到真实作者主页，并读取主页显示名称", () => {
    document.body.innerHTML = '<div class="user-profile-head-name"><div>公开作者</div><div>码龄22年</div></div><article class="blog-list-box"><a href="https://example.blog.csdn.net/article/details/123456"><h4>公开教程</h4><div class="blog-list-content">公开摘要</div><span>阅读 68 点赞 2 收藏 0</span></a></article>';
    const list = capture("csdn", "https://blog.csdn.net/example");
    expect(list.works[0]?.authorUrl).toBe("https://blog.csdn.net/example");
    expect(list.works[0]?.author).toBe("公开作者");
    expect(list.works[0]?.excerpt).toBe("公开摘要");
    document.body.innerHTML = '<h1 class="title-article">公开教程</h1><div id="content_views">公开正文</div>';
    const detail = capture("csdn", "https://example.blog.csdn.net/article/details/123456");
    expect(detail.works[0]?.authorUrl).toBe("https://blog.csdn.net/example");
  });

  it("同一搜索作品去重，读取小红书公开内容字段而不读取登录对象", () => {
    Reflect.set(window, "__INITIAL_STATE__", {search:{notes:[{id:"abcdef123456789012345678",xsecToken:"public-link-token",noteCard:{displayTitle:"公开笔记",user:{userId:"abcdef123456789012345670",nickname:"公开作者"},interactInfo:{likedCount:"2.1万",collectedCount:0},desc:"公开正文"}}]},account:{noteCard:{id:"abcdef123456789012345679",displayTitle:"不得读取的账号对象"}}});
    document.body.innerHTML = '<section class="note-item"><a href="https://www.xiaohongshu.com/explore/abcdef123456789012345678"><span class="title">公开笔记</span></a><span class="like-count">2.1万</span></section>';
    const result = capture("xiaohongshu", "https://www.xiaohongshu.com/search_result?keyword=办公");
    expect(result.works).toHaveLength(1);
    expect(result.works[0]?.metrics.views).toBeNull();
    expect(result.works[0]?.metrics.likes).toBe(21000);
    expect(result.works[0]?.metrics.favorites).toBe(0);
    expect(result.works[0]?.author).toBe("公开作者");
    expect(result.works[0]?.url).toContain("xsec_token=public-link-token");
  });

  it("多篇长正文分批传输，不丢掉后续作品", () => {
    Reflect.set(window, "__INITIAL_STATE__", {search:{notes:Array.from({length:8},(_,index)=>({id:`abcdef12345678901234567${index}`,displayTitle:`公开笔记${index}`,desc:"正文".repeat(12000)}))}});
    const result = capture("xiaohongshu", "https://www.xiaohongshu.com/search_result?keyword=办公");
    expect(result.chunkCount).toBeGreaterThan(1);
    expect(result.works.length).toBeGreaterThan(0);
    expect(result.works[0]?.body.length).toBe(24000);
    const all = [...result.works];
    for (let index = 1; index < result.chunkCount; index++) all.push(...result.next());
    expect(new Set(all.map(item => item.title)).size).toBe(8);
  });

  it("读取新版小红书可见标题链接，并在搜索笔记详情补全正文与互动", () => {
    const id = "6a89a86d0000000033008dae";
    Reflect.set(window, "__INITIAL_STATE__", {search:{notes:[{id,noteCard:{displayTitle:"公开笔记",interactInfo:{likedCount:"3320"}}}]}});
    document.body.innerHTML = `<section class="note-item"><a href="https://www.xiaohongshu.com/explore/${id}"></a><a class="title" href="https://www.xiaohongshu.com/search_result/${id}?xsec_token=public-link-token">公开笔记</a><a class="author" href="https://www.xiaohongshu.com/user/profile/6a832aa90000000013031c0b"><div class="name-time-wrapper"><div class="name">公开作者</div><div class="time">08-22</div></div></a><span class="like-wrapper"><span class="count">3320</span></span></section>`;
    const list = capture("xiaohongshu", "https://www.xiaohongshu.com/search_result/?keyword=办公");
    expect(list.works).toHaveLength(1);
    expect(list.works[0]?.author).toBe("公开作者");
    expect(list.works[0]?.url).toContain("xsec_token=public-link-token");
    Reflect.deleteProperty(window, "__INITIAL_STATE__");
    document.body.innerHTML = '<h1 id="detail-title">公开笔记</h1><div id="detail-desc">公开笔记完整正文</div><div class="interact-container"><span class="like-wrapper"><span class="count">3320</span></span><span class="collect-wrapper"><span class="count">0</span></span><span class="chat-wrapper"><span class="count">18</span></span></div>';
    const detail = capture("xiaohongshu", `https://www.xiaohongshu.com/search_result/${id}?xsec_token=public-link-token`);
    expect(detail.works[0]?.body).toBe("公开笔记完整正文");
    expect(detail.works[0]?.metrics).toEqual({views:null,likes:3320,favorites:0,comments:18,shares:null});
  });
});
