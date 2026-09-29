import { MantineProvider } from "@mantine/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { createInstance } from "i18next";
import { createStore, Provider } from "jotai";
import type { ReactElement } from "react";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import zhCN from "./i18n/zh-CN.json";
import type { BenchmarkWork, ResearchData } from "./researchTypes";

/** 只模拟宿主边界，保留页面真实交互与Query契约。 */
const mocks = vi.hoisted(() => ({getResearchLibrary:vi.fn(),getCollectionStatus:vi.fn(),startCollection:vi.fn(),listProviderConfigs:vi.fn(),generateContent:vi.fn(),analyzeStyle:vi.fn(),updateStyle:vi.fn(),updatePrompts:vi.fn(),updateDraft:vi.fn(),exportDraft:vi.fn(),navigate:vi.fn()}));
vi.mock("./researchCommands", () => ({...mocks,researchError:()=>"测试错误"}));
vi.mock("../workbench/workbenchCommands", () => ({listProviderConfigs:mocks.listProviderConfigs}));
vi.mock("@tanstack/react-router", () => ({useNavigate:()=>mocks.navigate}));
import { BenchmarkPage } from "./BenchmarkPage";
import { StyleLibraryPage } from "./StyleLibraryPage";
import { WriterPage } from "./WriterPage";
import { benchmarkSessionAtom, composerSessionAtom } from "./researchState";

/** 构造隔离的单元测试作品，不作为实际采集结果或产品演示数据。 */
function testWork(id: string, views: number | null): BenchmarkWork {
  return {id,platform:"csdn",title:`作品${id}`,url:`https://blog.csdn.net/test/article/details/${id}`,author:"测试作者",authorUrl:"https://blog.csdn.net/test",publishedAt:"",excerpt:"",body:"正文材料".repeat(40),metrics:{views,likes:0,favorites:null,comments:null,shares:null},collectedAtMs:100};
}

/** 在同一根store下渲染页面，验证跨路由选择状态。 */
async function renderPage(page: ReactElement, store = createStore()) {
  const i18n = createInstance();
  await i18n.init({lng:"zh-CN",resources:{"zh-CN":{research:zhCN}}});
  const client = new QueryClient({defaultOptions:{queries:{retry:false}}});
  return {...render(<I18nextProvider i18n={i18n}><Provider store={store}><QueryClientProvider client={client}><MantineProvider env="test">{page}</MantineProvider></QueryClientProvider></Provider></I18nextProvider>),store,client};
}

beforeEach(() => {
  for (const mock of Object.values(mocks)) mock.mockReset();
  const data: ResearchData = {works:[testWork("123",null),testWork("124",1200),testWork("125",20)],styles:[],drafts:[],prompts:{seoRewrite:"原创改写",xhsRewrite:"原创笔记",styleAnalysis:"分析风格"}};
  mocks.getResearchLibrary.mockResolvedValue(data);
  mocks.getCollectionStatus.mockResolvedValue({pageUrl:"",message:"",collectedCount:0,updatedAtMs:0});
  mocks.listProviderConfigs.mockResolvedValue([{id:"text-provider",name:"已保存文本服务",enabled:true,kind:"openAiCompatible",hasApiKey:true,articleModel:"test-model",defaultForArticle:true}]);
  Object.defineProperty(window,"matchMedia",{configurable:true,value:vi.fn().mockImplementation(()=>({matches:false,addEventListener:vi.fn(),removeEventListener:vi.fn()}))});
  vi.stubGlobal("ResizeObserver",class {observe() {} unobserve() {} disconnect() {}});
  Object.defineProperty(Element.prototype,"scrollIntoView",{configurable:true,value:vi.fn()});
  Object.defineProperty(document,"fonts",{configurable:true,value:{addEventListener:vi.fn(),removeEventListener:vi.fn()}});
});
afterEach(() => {cleanup();vi.unstubAllGlobals();});

describe("研究页面契约", () => {
  it("行和单元格不代理勾选，最低指标排除缺失值", async () => {
    await renderPage(<BenchmarkPage/>);
    const checkbox = await screen.findByRole("checkbox",{name:"选择 作品124"});
    fireEvent.click(checkbox.closest("tr")!);
    expect(checkbox).not.toBeChecked();
    fireEvent.click(checkbox);
    expect(checkbox).toBeChecked();
    fireEvent.change(screen.getByLabelText("最低阅读／播放"),{target:{value:"100"}});
    await waitFor(()=>expect(screen.queryByRole("checkbox",{name:"选择 作品123"})).not.toBeInTheDocument());
    expect(screen.queryByRole("checkbox",{name:"选择 作品125"})).not.toBeInTheDocument();
    expect(checkbox).toBeInTheDocument();
  });

  it("同一根store跨页面保留选择，新进程store恢复默认", async () => {
    const first = await renderPage(<BenchmarkPage/>);
    fireEvent.click(await screen.findByRole("checkbox",{name:"选择 作品124"}));
    first.unmount();first.client.clear();
    const second = await renderPage(<BenchmarkPage/>,first.store);
    expect(await screen.findByRole("checkbox",{name:"选择 作品124"})).toBeChecked();
    second.unmount();second.client.clear();
    await renderPage(<BenchmarkPage/>);
    expect(await screen.findByRole("checkbox",{name:"选择 作品124"})).not.toBeChecked();
  });

  it("使用原有扁平ProviderConfigView支持生成与三篇风格分析", async () => {
    const store = createStore();
    store.set(composerSessionAtom,{...store.get(composerSessionAtom),keyword:"Python 自动化"});
    const writer = await renderPage(<WriterPage/>,store);
    await waitFor(()=>expect(screen.getByRole("button",{name:"生成二次标题和正文"})).toBeEnabled());
    writer.unmount();writer.client.clear();
    store.set(benchmarkSessionAtom,{...store.get(benchmarkSessionAtom),selected:["123","124","125"]});
    await renderPage(<StyleLibraryPage/>,store);
    await waitFor(()=>expect(screen.getByRole("button",{name:"分析并保存风格"})).toBeEnabled());
  });
});
