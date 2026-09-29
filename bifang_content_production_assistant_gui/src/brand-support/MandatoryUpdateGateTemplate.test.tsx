import { MantineProvider } from "@mantine/core";
import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { MandatoryUpdateGateTemplate } from "./MandatoryUpdateGateTemplate";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

describe("mandatory update root gate", () => {
  afterEach(cleanup);

  beforeEach(() => {
    Object.defineProperty(window, "matchMedia", {
      configurable: true,
      value: vi.fn().mockImplementation((query: string) => ({
        addEventListener: vi.fn(),
        addListener: vi.fn(),
        dispatchEvent: vi.fn(),
        matches: false,
        media: query,
        onchange: null,
        removeEventListener: vi.fn(),
        removeListener: vi.fn(),
      })),
    });
  });

  /** 强更状态必须完全替换普通产品内容，只保留下载与退出。 */
  it("blocks product content for a required update", () => {
    const onExitApplication = vi.fn();
    const onOpenUpdateDownload = vi.fn();
    render(
      <MantineProvider>
        <MandatoryUpdateGateTemplate
          onExitApplication={onExitApplication}
          onOpenUpdateDownload={onOpenUpdateDownload}
          update={{
            currentVersion: "0.2.17",
            downloadReady: true,
            message: "请更新后继续。",
            status: "required-update",
            title: "发现新版本",
          }}
        >
          <div>product-content</div>
        </MandatoryUpdateGateTemplate>
      </MantineProvider>,
    );

    expect(screen.queryByText("product-content")).not.toBeInTheDocument();
    fireEvent.click(
      screen.getByRole("button", { name: "updater.open_download" }),
    );
    fireEvent.click(
      screen.getByRole("button", { name: "updater.exit_application" }),
    );
    expect(onOpenUpdateDownload).toHaveBeenCalledOnce();
    expect(onExitApplication).toHaveBeenCalledOnce();
  });

  /** 非强更状态不得阻断本地功能。 */
  it("keeps product content available for optional updates", () => {
    render(
      <MantineProvider>
        <MandatoryUpdateGateTemplate
          onExitApplication={vi.fn()}
          onOpenUpdateDownload={vi.fn()}
          update={{
            currentVersion: "0.2.17",
            downloadReady: true,
            status: "optional-update",
          }}
        >
          <div>product-content</div>
        </MandatoryUpdateGateTemplate>
      </MantineProvider>,
    );

    expect(screen.getByText("product-content")).toBeInTheDocument();
    expect(
      screen.queryByTestId("mandatory-update-gate"),
    ).not.toBeInTheDocument();
  });
});
