import { createFileRoute } from "@tanstack/react-router";
import type { ReactElement } from "react";

import { SponsorPageTemplate } from "../brand-support/SponsorPageTemplate";

/** 渲染完整本地品牌赞助页面，不产生交易或网络副作用。 */
function SponsorRouteComponent(): ReactElement {
  return <SponsorPageTemplate />;
}

export const Route = createFileRoute("/sponsor")({
  component: SponsorRouteComponent,
});
