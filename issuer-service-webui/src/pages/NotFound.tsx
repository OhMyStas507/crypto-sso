import { A } from "@solidjs/router";

export function NotFound() {
  return (
    <div class="centered">
      <h1>页面不存在</h1>
      <p>检查访问地址，或返回管理概览。</p>
      <A href="/overview" class="btn primary">
        返回概览
      </A>
    </div>
  );
}
