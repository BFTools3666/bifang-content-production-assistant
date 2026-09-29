import { createFileRoute } from "@tanstack/react-router";
import { BenchmarkPage } from "../research/BenchmarkPage";

/** 对标作品与公开数据筛选的固定本地路由。 */
export const Route = createFileRoute("/benchmarks")({ component: BenchmarkPage });
