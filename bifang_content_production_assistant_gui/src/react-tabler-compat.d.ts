import "react";

declare module "react" {
  /** 为 Tabler 3.46 的 SVG 节点声明补齐 React 类型命名空间。 */
  interface ReactSVG {
    [elementName: string]: SVGElement;
  }
}
