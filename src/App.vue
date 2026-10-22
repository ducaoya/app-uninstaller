<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AppInfo, Report } from "./types";

const apps = ref<AppInfo[]>([]);
const loading = ref(false);
const busy = ref(false);
const search = ref("");
const selectedId = ref("");
const menuOpen = ref(false);
const menuX = ref(0);
const menuY = ref(0);
const logLines = ref<string[]>([]);
const lastSummary = ref("");
const logBox = ref<HTMLElement | null>(null);
let unlisten: UnlistenFn | null = null;

const selected = computed<AppInfo | undefined>(() =>
  apps.value.find((a) => a.id === selectedId.value),
);

const filtered = computed<AppInfo[]>(() => {
  const q = search.value.trim().toLowerCase();
  if (!q) return apps.value;
  return apps.value.filter((a) =>
    `${a.name} ${a.publisher} ${a.version}`.toLowerCase().includes(q),
  );
});

async function refresh(): Promise<void> {
  loading.value = true;
  try {
    apps.value = await invoke<AppInfo[]>("list_apps");
    log(`列表已刷新，共 ${apps.value.length} 个应用`);
  } catch (e) {
    log(`[错误] 加载列表失败: ${e}`);
  } finally {
    loading.value = false;
  }
}

function sizeText(kb: number): string {
  if (!kb) return "—";
  if (kb < 1024) return `${kb} KB`;
  return `${(kb / 1024).toFixed(1)} MB`;
}

function log(line: string): void {
  logLines.value.push(line);
  if (logLines.value.length > 500) logLines.value.splice(0, logLines.value.length - 500);
  requestAnimationFrame(() => {
    logBox.value?.scrollTo({ top: logBox.value.scrollHeight });
  });
}

function openMenu(a: AppInfo, ev: MouseEvent): void {
  selectedId.value = a.id;
  menuX.value = Math.min(ev.clientX, window.innerWidth - 230);
  menuY.value = Math.min(ev.clientY, window.innerHeight - 200);
  menuOpen.value = true;
}

function closeMenu(): void {
  menuOpen.value = false;
}

async function runJob(cmd: string, label: string, a: AppInfo): Promise<void> {
  closeMenu();
  if (busy.value) return;
  busy.value = true;
  lastSummary.value = "";
  log(`—— ${label}：${a.name} ——`);
  try {
    const rep = await invoke<Report>(cmd, { id: a.id });
    const marks: Record<string, string> = { done: "✓", not_found: "·", skipped: "–", error: "✗" };
    for (const s of rep.steps) {
      log(`${marks[s.status] ?? "✗"} ${s.action} ${s.target}${s.detail ? `（${s.detail}）` : ""}`);
    }
    log(`（${label}结束，共 ${rep.steps.length} 项操作）`);
    lastSummary.value = rep.ok
      ? `${label}完成，${rep.steps.length} 项操作，详见日志`
      : `${label}完成，但存在失败项，详见日志中 ✗ 条目`;
    if (cmd === "uninstall_app" || cmd === "force_uninstall") await refresh();
  } catch (e) {
    log(`[错误] ${e}`);
    lastSummary.value = `${label}失败：${e}`;
  } finally {
    busy.value = false;
  }
}

function uninstall(a: AppInfo): void {
  closeMenu();
  runJob("uninstall_app", "卸载", a);
}

function forceUninstall(a: AppInfo): void {
  closeMenu();
  // 强制卸载不可逆，二次确认
  if (window.confirm(`强制卸载「${a.name}」？\n\n将直接删除安装目录、注册表键并清理残留，此操作不可恢复（数据目录进回收站）。`)) {
    runJob("force_uninstall", "强制卸载", a);
  }
}

onMounted(async () => {
  unlisten = await listen<string>("job-log", (e) => log(e.payload));
  await refresh();
});
onUnmounted(() => unlisten?.());
</script>

<template>
  <div class="app" @click="closeMenu" @contextmenu.prevent="closeMenu">
    <header class="top">
      <h1>应用卸载器</h1>
      <span class="count">{{ filtered.length }} / {{ apps.length }} 个应用</span>
      <input v-model="search" class="search" placeholder="搜索应用…" />
      <button :disabled="loading || busy" @click="refresh">刷新</button>
    </header>

    <main class="main">
      <section class="list">
        <div v-if="loading" class="hint">正在扫描注册表…</div>
        <div v-else-if="filtered.length === 0" class="hint">无匹配应用</div>
        <div v-else class="rows">
          <div
            v-for="a in filtered"
            :key="a.id"
            class="row"
            :class="{ sel: a.id === selectedId }"
            @click="selectedId = a.id"
            @contextmenu.prevent.stop="openMenu(a, $event)"
          >
            <span class="badge">{{ a.name.slice(0, 1).toUpperCase() }}</span>
            <span class="meta">
              <span class="name">
                {{ a.name }}
                <i v-if="a.isMsi" class="msi">MSI</i>
              </span>
              <span class="sub">{{ a.publisher || "未知发布者" }} · {{ a.version || "—" }}</span>
            </span>
            <span class="size">{{ sizeText(a.sizeKb) }}</span>
          </div>
        </div>
      </section>

      <aside class="log-panel">
        <h2>操作日志</h2>
        <div v-if="lastSummary" class="summary" :class="{ bad: lastSummary.includes('失败') }">
          {{ lastSummary }}
        </div>
        <div ref="logBox" class="lines">
          <div v-for="(l, i) in logLines" :key="i" class="line">{{ l }}</div>
          <div v-if="logLines.length === 0" class="hint">右键应用执行操作，日志显示在这里</div>
        </div>
      </aside>
    </main>

    <div v-if="menuOpen && selected" class="menu" :style="{ left: menuX + 'px', top: menuY + 'px' }" @click.stop>
      <button :disabled="!selected.uninstallString || busy" class="mi" @click="uninstall(selected)">
        卸载
      </button>
      <button :disabled="busy" class="mi danger" @click="forceUninstall(selected)">强制卸载</button>
      <button :disabled="busy" class="mi" @click="runJob('clean_registry', '清除注册表', selected)">
        清除注册表
      </button>
      <button :disabled="busy" class="mi" @click="runJob('clean_cache', '清除缓存', selected)">
        清除缓存文件夹
      </button>
    </div>

    <div v-if="busy" class="busy-mask">处理中，请稍候…</div>
  </div>
</template>

<style scoped>
.app {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.top {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 14px;
  border-bottom: 1px solid #262c3a;
}

.top h1 {
  font-size: 15px;
  margin: 0;
}

.count {
  color: #8b94a7;
}

.search {
  margin-left: auto;
  width: 220px;
}

.main {
  flex: 1;
  display: flex;
  min-height: 0;
}

.list {
  flex: 1.5;
  overflow: auto;
  border-right: 1px solid #262c3a;
}

.rows {
  padding: 6px;
}

.row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 7px 10px;
  border-radius: 6px;
  cursor: context-menu;
}

.row:hover {
  background: #1d2434;
}

.row.sel {
  background: #24304a;
}

.badge {
  width: 30px;
  height: 30px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 7px;
  background: #2b3550;
  color: #9db4ff;
  font-weight: 600;
  flex-shrink: 0;
}

.meta {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.name {
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.msi {
  font-style: normal;
  font-size: 10px;
  background: #3a3520;
  color: #e5c07b;
  border-radius: 4px;
  padding: 1px 5px;
  margin-left: 4px;
}

.sub {
  color: #8b94a7;
  font-size: 12px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.size {
  color: #8b94a7;
  flex-shrink: 0;
}

.log-panel {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 300px;
  padding: 10px 14px;
}

.log-panel h2 {
  font-size: 13px;
  margin: 0 0 8px;
  color: #8b94a7;
}

.summary {
  padding: 8px 10px;
  border-radius: 6px;
  background: #1d3327;
  color: #7fd88f;
  margin-bottom: 8px;
}

.summary.bad {
  background: #3a2226;
  color: #e08c96;
}

.lines {
  flex: 1;
  overflow: auto;
  background: #10131a;
  border: 1px solid #262c3a;
  border-radius: 6px;
  padding: 8px 10px;
  font-family: Consolas, monospace;
  font-size: 12px;
}

.line {
  white-space: pre-wrap;
  word-break: break-all;
  line-height: 1.6;
}

.hint {
  color: #5b6377;
  padding: 14px;
}

.menu {
  position: fixed;
  z-index: 10;
  display: flex;
  flex-direction: column;
  background: #1d2230;
  border: 1px solid #3a4256;
  border-radius: 8px;
  padding: 4px;
  min-width: 180px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
}

.mi {
  text-align: left;
  background: none;
  border: none;
  padding: 8px 12px;
  border-radius: 5px;
}

.mi:hover:not(:disabled) {
  background: #2b3550;
}

.mi.danger {
  color: #e08c96;
}

.busy-mask {
  position: fixed;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(10, 12, 18, 0.55);
  z-index: 20;
  font-size: 15px;
}
</style>
