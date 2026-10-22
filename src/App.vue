<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { AppInfo, LogLine, Report } from "./types";
import LogIcon from "./LogIcon.vue";

interface Toast {
  kind: "ok" | "err";
  text: string;
}

interface UpdateInfo {
  current: string;
  latest: string;
  hasUpdate: boolean;
  url: string;
  notes: string;
}

const win = getCurrentWindow();

const apps = ref<AppInfo[]>([]);
const icons = ref<Record<string, string>>({});
const loading = ref(false);
const busy = ref(false);
const search = ref("");
const scope = ref<"*" | "sys" | "wow64" | "user">("*");
const maximized = ref(false);

const menuOpen = ref(false);
const menuX = ref(0);
const menuY = ref(0);
const menuApp = ref<AppInfo | null>(null);

const logEntries = ref<LogLine[]>([]);
const statusText = ref("就绪 · 右键应用执行操作");
const statusLevel = ref<LogLine["level"]>("info");
const view = ref<"apps" | "about">("apps");
const appVersion = ref("0.1.0");
const checking = ref(false);
const updateMsg = ref("");
const updateInfo = ref<UpdateInfo | null>(null);
const logMode = ref<"all" | "core">("all");
const currentOp = ref<{ text: string; start: number } | null>(null);
const nowTick = ref(Date.now());
const busySecs = ref(0);

const logExpanded = ref(false);
const pendingForce = ref<AppInfo | null>(null);
const toast = ref<Toast | null>(null);
const logBox = ref<HTMLElement | null>(null);

let unlistenLog: UnlistenFn | null = null;
let unlistenResized: UnlistenFn | null = null;
let toastTimer: ReturnType<typeof setTimeout> | null = null;
let busyTimer: ReturnType<typeof setInterval> | null = null;
let tickTimer: ReturnType<typeof setInterval> | null = null;

/* ---------- 数据 ---------- */

async function refresh(silent = false): Promise<void> {
  loading.value = true;
  try {
    apps.value = await invoke<AppInfo[]>("list_apps");
    if (!silent) say("ok", `已加载 ${apps.value.length} 个应用`);
    void loadIcons();
  } catch (e) {
    say("err", `加载列表失败：${e}`);
    addLog("err", `[错误] 加载列表失败: ${e}`);
  } finally {
    loading.value = false;
  }
}

async function loadIcons(): Promise<void> {
  const keys = [...new Set(apps.value.map((a) => a.iconKey).filter(Boolean))];
  if (!keys.length) return;
  try {
    const map = await invoke<Record<string, string>>("load_icons", { keys });
    icons.value = { ...icons.value, ...map };
  } catch {
    /* 图标加载失败静默回退字母块 */
  }
}

function addLog(level: LogLine["level"], text: string): void {
  if (level === "phase") {
    // 步骤开始：进入“进行中”槽位，完成后被结果行替换
    currentOp.value = { text, start: Date.now() };
    statusText.value = text;
    return;
  }
  currentOp.value = null;
  logEntries.value.push({ level, text });
  if (logEntries.value.length > 800) {
    logEntries.value.splice(0, logEntries.value.length - 800);
  }
  statusText.value = text;
  statusLevel.value = level;
  requestAnimationFrame(() => {
    logBox.value?.scrollTo({ top: logBox.value.scrollHeight });
  });
}

function fmtLive(ms: number): string {
  const s = ms / 1000;
  if (s < 60) return `${s.toFixed(1)}s`;
  return `${Math.floor(s / 60)}m${String(Math.floor(s % 60)).padStart(2, "0")}s`;
}

const opElapsed = computed(() =>
  currentOp.value ? fmtLive(nowTick.value - currentOp.value.start) : "",
);

const busyText = computed(() => {
  const m = Math.floor(busySecs.value / 60);
  const s = busySecs.value % 60;
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
});

watch(busy, (b) => {
  if (b) {
    busySecs.value = 0;
    busyTimer = setInterval(() => busySecs.value++, 1000);
  } else if (busyTimer) {
    clearInterval(busyTimer);
    busyTimer = null;
  }
});

const visibleLog = computed<LogLine[]>(() =>
  logMode.value === "core"
    ? logEntries.value.filter((e) => e.level !== "info")
    : logEntries.value,
);

/* ---------- 过滤 / 分组 ---------- */

function hiveOf(a: AppInfo): string {
  return a.id.split("|")[0] ?? "";
}

const scoped = computed<AppInfo[]>(() => {
  const s = scope.value;
  return apps.value.filter((a) => {
    const h = hiveOf(a);
    if (s === "sys") return h === "HKLM" || h === "WOW64";
    if (s === "wow64") return h === "WOW64";
    if (s === "user") return h === "HKCU";
    return true;
  });
});

const filtered = computed<AppInfo[]>(() => {
  const q = search.value.trim().toLowerCase();
  if (!q) return scoped.value;
  return scoped.value.filter((a) =>
    `${a.name} ${a.publisher} ${a.version}`.toLowerCase().includes(q),
  );
});

const groups = computed<Array<{ key: string; label: string; apps: AppInfo[] }>>(() => {
  const defs: Array<{ key: string; label: string; match: (h: string) => boolean }> = [
    { key: "hklm", label: "系统级 · 所有用户", match: (h) => h === "HKLM" },
    { key: "wow64", label: "系统级 · 32 位", match: (h) => h === "WOW64" },
    { key: "hkcu", label: "当前用户", match: (h) => h === "HKCU" },
  ];
  const out: Array<{ key: string; label: string; apps: AppInfo[] }> = [];
  for (const d of defs) {
    const list = filtered.value.filter((a) => d.match(hiveOf(a)));
    if (list.length) out.push({ key: d.key, label: d.label, apps: list });
  }
  return out;
});

const counts = computed<Record<string, number>>(() => {
  const c: Record<string, number> = { "*": apps.value.length, sys: 0, wow64: 0, user: 0 };
  for (const a of apps.value) {
    const h = hiveOf(a);
    if (h === "HKLM" || h === "WOW64") c.sys++;
    if (h === "WOW64") c.wow64++;
    if (h === "HKCU") c.user++;
  }
  return c;
});

const totalSizeKb = computed(() => apps.value.reduce((n, a) => n + a.sizeKb, 0));

function sizeText(kb: number): string {
  if (!kb) return "—";
  if (kb < 1024) return `${kb} KB`;
  if (kb < 1024 * 1024) return `${(kb / 1024).toFixed(1)} MB`;
  return `${(kb / 1024 / 1024).toFixed(2)} GB`;
}

/* ---------- 窗口控制 ---------- */

async function toggleMax(): Promise<void> {
  await win.toggleMaximize();
  maximized.value = await win.isMaximized();
}

/* ---------- toast ---------- */

function say(kind: "ok" | "err", text: string): void {
  toast.value = { kind, text };
  if (toastTimer) clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (toast.value = null), 3200);
}

/* ---------- 右键菜单 ---------- */

function openMenu(a: AppInfo, ev: MouseEvent): void {
  menuApp.value = a;
  menuX.value = Math.min(ev.clientX, window.innerWidth - 230);
  menuY.value = Math.min(ev.clientY, window.innerHeight - 240);
  menuOpen.value = true;
}

function closeMenu(): void {
  menuOpen.value = false;
}

function setScope(s: "*" | "sys" | "wow64" | "user"): void {
  scope.value = s;
  view.value = "apps";
}

/* ---------- 关于 / 检查更新 ---------- */

async function checkUpdate(): Promise<void> {
  if (checking.value) return;
  checking.value = true;
  updateMsg.value = "";
  updateInfo.value = null;
  try {
    const u = await invoke<UpdateInfo>("check_update");
    updateInfo.value = u;
    updateMsg.value = u.hasUpdate
      ? `发现新版本 v${u.latest}`
      : `已是最新版本（v${u.current}）`;
  } catch (e) {
    updateInfo.value = null;
    updateMsg.value = "";
    say("err", String(e));
  } finally {
    checking.value = false;
  }
}

async function goDownload(): Promise<void> {
  if (!updateInfo.value?.url) return;
  try {
    await invoke("open_url", { url: updateInfo.value.url });
  } catch (e) {
    say("err", String(e));
  }
}

/* ---------- 任务执行（处理中互斥）---------- */

async function runJob(cmd: string, label: string, a: AppInfo): Promise<void> {
  closeMenu();
  if (busy.value) {
    say("err", "有任务进行中，请等待完成或先取消");
    return;
  }
  busy.value = true;
  // 新任务重新开始日志
  logEntries.value = [];
  currentOp.value = null;
  try {
    const rep = await invoke<Report>(cmd, { id: a.id });
    say(
      rep.ok ? "ok" : "err",
      rep.ok ? `${label}完成 · 总耗时 ${busyText.value}` : `${label}存在失败项，详见日志`,
    );
    if (cmd === "uninstall_app" || cmd === "force_uninstall") await refresh(true);
  } catch (e) {
    addLog("err", `[错误] ${e}`);
    say("err", `${label}失败：${e}`);
  } finally {
    busy.value = false;
  }
}

async function cancelJob(): Promise<void> {
  if (!busy.value) return;
  try {
    await invoke("cancel_job");
    say("ok", "已请求取消，正在停止…");
  } catch (e) {
    say("err", `取消失败：${e}`);
  }
}

function uninstall(a: AppInfo): void {
  closeMenu();
  void runJob("uninstall_app", "卸载", a);
}

function askForce(a: AppInfo): void {
  closeMenu();
  pendingForce.value = a;
}

function confirmForce(): void {
  const a = pendingForce.value;
  pendingForce.value = null;
  if (a) void runJob("force_uninstall", "强制卸载", a);
}

function cleanRegistry(a: AppInfo): void {
  closeMenu();
  void runJob("clean_registry", "清除注册表", a);
}

function cleanCache(a: AppInfo): void {
  closeMenu();
  void runJob("clean_cache", "清除缓存", a);
}

/* ---------- 生命周期 ---------- */

function onKeydown(e: KeyboardEvent): void {
  if (e.key === "Escape") {
    closeMenu();
    logExpanded.value = false;
    pendingForce.value = null;
  }
}

onMounted(async () => {
  unlistenLog = await listen<LogLine>("job-log", (e) => addLog(e.payload.level, e.payload.text));
  unlistenResized = await win.onResized(async () => {
    maximized.value = await win.isMaximized();
  });
  window.addEventListener("keydown", onKeydown);
  tickTimer = setInterval(() => (nowTick.value = Date.now()), 200);
  getVersion()
    .then((v) => (appVersion.value = v))
    .catch(() => {});
  await refresh(true);
  addLog("ok", `列表加载完成，共 ${apps.value.length} 个应用`);
});

onUnmounted(() => {
  unlistenLog?.();
  unlistenResized?.();
  window.removeEventListener("keydown", onKeydown);
  if (toastTimer) clearTimeout(toastTimer);
  if (busyTimer) clearInterval(busyTimer);
  if (tickTimer) clearInterval(tickTimer);
});
</script>

<template>
  <div class="app" :class="{ maximized }" @click="closeMenu" @contextmenu.prevent="closeMenu">
    <!-- 标题栏 -->
    <header class="titlebar" data-tauri-drag-region @dblclick="toggleMax">
      <div class="brand" data-tauri-drag-region>
        <span class="brand-dot" data-tauri-drag-region />
        <span class="brand-name" data-tauri-drag-region>应用卸载器</span>
      </div>
      <div class="window-actions">
        <button class="win-btn" title="最小化" @click="win.minimize()">
          <svg width="10" height="10" viewBox="0 0 10 10">
            <path d="M2 5h6" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
          </svg>
        </button>
        <button class="win-btn" :title="maximized ? '还原' : '最大化'" @click="toggleMax">
          <svg width="10" height="10" viewBox="0 0 10 10">
            <rect x="2.4" y="2.4" width="5.2" height="5.2" rx="1.4" stroke="currentColor" stroke-width="1.1" fill="none" />
          </svg>
        </button>
        <button class="win-btn close" title="关闭" @click="win.close()">
          <svg width="10" height="10" viewBox="0 0 10 10">
            <path d="M2.6 2.6l4.8 4.8M7.4 2.6L2.6 7.4" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
          </svg>
        </button>
      </div>
    </header>

    <div class="body">
      <!-- 侧栏 -->
      <aside class="sidebar">
        <nav class="nav">
          <button class="nav-item" :class="{ active: view === 'apps' && scope === '*' }" @click="setScope('*')">
            <span>全部应用</span>
            <em>{{ counts["*"] }}</em>
          </button>
          <button class="nav-item" :class="{ active: view === 'apps' && scope === 'sys' }" @click="setScope('sys')">
            <span>系统级</span>
            <em>{{ counts.sys }}</em>
          </button>
          <button class="nav-item" :class="{ active: view === 'apps' && scope === 'wow64' }" @click="setScope('wow64')">
            <span>32 位软件</span>
            <em>{{ counts.wow64 }}</em>
          </button>
          <button class="nav-item" :class="{ active: view === 'apps' && scope === 'user' }" @click="setScope('user')">
            <span>当前用户</span>
            <em>{{ counts.user }}</em>
          </button>
        </nav>
        <div class="side-group">
          <div class="side-row">
            <span>应用总数</span>
            <span class="side-value">{{ apps.length }}</span>
          </div>
          <div class="side-row">
            <span>占用空间</span>
            <span class="side-value">{{ sizeText(totalSizeKb) }}</span>
          </div>
          <div class="side-row">
            <span>运行权限</span>
            <span class="side-value">管理员</span>
          </div>
        </div>
        <button class="nav-item about-nav" :class="{ active: view === 'about' }" @click="view = 'about'">
          <span>关于</span>
        </button>
      </aside>

      <!-- 主区 -->
      <section class="main">
        <template v-if="view === 'about'">
          <div class="about">
            <img src="/icon.png" class="about-icon" alt="icon" />
            <h1 class="about-title">应用卸载器</h1>
            <div class="about-ver">v{{ appVersion }}</div>
            <p class="about-desc">
              扫描注册表卸载信息，安全卸载应用并自动清理残留（注册表、数据目录、快捷方式）。
            </p>
            <div class="about-update">
              <button class="pill-btn solid" :disabled="checking" @click="checkUpdate">
                {{ checking ? "检查中…" : "检查更新" }}
              </button>
              <span v-if="updateMsg" class="update-result ok">{{ updateMsg }}</span>
            </div>
            <div v-if="updateInfo?.hasUpdate" class="update-card">
              <div class="update-line">新版本 v{{ updateInfo.latest }}（当前 v{{ updateInfo.current }}）</div>
              <p v-if="updateInfo.notes" class="update-notes">{{ updateInfo.notes }}</p>
              <button class="pill-btn solid" @click="goDownload">前往 GitHub 下载</button>
            </div>
            <div class="about-group">
              <div class="side-row">
                <span>开源协议</span>
                <span class="side-value">MIT License</span>
              </div>
              <div class="side-row">
                <span>源代码</span>
                <span class="side-value">GitHub（上传后可用）</span>
              </div>
            </div>
            <div class="about-copy">© 2026 ducaoya · 涉及注册表与文件删除，请谨慎操作</div>
          </div>
        </template>
        <template v-else>
        <div class="toolbar">
          <div class="search-wrap">
            <span class="search-icon">
              <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
                <circle cx="5.2" cy="5.2" r="3.7" stroke="currentColor" stroke-width="1.2" />
                <path d="M8.2 8.2l2.3 2.3" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
              </svg>
            </span>
            <input v-model="search" class="search" placeholder="搜索应用…" />
          </div>
          <span class="toolbar-tip">右键应用：卸载 / 强制卸载 / 清注册表 / 清缓存</span>
          <button class="pill-btn" :disabled="loading || busy" @click="refresh()">
            {{ loading ? "扫描中…" : "刷新" }}
          </button>
        </div>

        <div class="list">
          <div v-if="loading" class="empty">
            <div class="spinner" />
            正在扫描注册表…
          </div>
          <div v-else-if="filtered.length === 0" class="empty">
            {{ search ? "无匹配应用" : "未发现已安装应用" }}
          </div>
          <template v-else>
            <div v-for="g in groups" :key="g.key" class="group">
              <div class="group-title">{{ g.label }} · {{ g.apps.length }}</div>
              <div class="group-body">
                <div
                  v-for="a in g.apps"
                  :key="a.id"
                  class="row"
                  @contextmenu.prevent.stop="openMenu(a, $event)"
                >
                  <img v-if="icons[a.iconKey]" class="row-icon" :src="icons[a.iconKey]" alt="" />
                  <span v-else class="icon-fallback">{{ a.name.slice(0, 1).toUpperCase() }}</span>
                  <span class="row-main">
                    <span class="row-title-line">
                      <span class="row-title">{{ a.name }}</span>
                      <i v-if="a.isMsi" class="tag soft">MSI</i>
                    </span>
                    <span class="row-sub">{{ a.publisher || "未知发布者" }} · {{ a.version || "—" }}</span>
                  </span>
                  <span class="row-size">{{ sizeText(a.sizeKb) }}</span>
                  <span class="row-actions">
                    <button class="mini-btn" :disabled="busy || !a.uninstallString" @click.stop="uninstall(a)">
                      卸载
                    </button>
                    <button class="mini-btn danger" :disabled="busy" @click.stop="askForce(a)">强卸</button>
                  </span>
                </div>
              </div>
            </div>
          </template>
        </div>

        <!-- 日志抽屉（展开时占主区大部） -->
        <div v-if="logExpanded" class="log-drawer">
          <div class="log-head">
            <h2>操作日志</h2>
            <div class="segmented">
              <button class="seg" :class="{ active: logMode === 'all' }" @click="logMode = 'all'">全部</button>
              <button class="seg" :class="{ active: logMode === 'core' }" @click="logMode = 'core'">核心日志</button>
            </div>
            <button class="icon-btn" title="收起" @click="logExpanded = false">
              <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
                <path d="M2.5 7.5L6 4l3.5 3.5" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" />
              </svg>
            </button>
          </div>
          <div ref="logBox" class="log-stream">
            <template v-for="(e, i) in visibleLog" :key="i">
              <div v-if="e.level === 'info'" class="log-entry detail">{{ e.text }}</div>
              <div v-else class="log-entry core" :class="e.level">
                <span class="log-icon"><LogIcon :level="e.level" /></span>
                <span>{{ e.text }}</span>
              </div>
            </template>
            <div v-if="visibleLog.length === 0" class="log-entry detail">暂无日志</div>
            <div v-if="currentOp" class="log-entry running">
              <span class="spinner tiny" />
              <span class="running-text">{{ currentOp.text }}</span>
              <span class="running-time">{{ opElapsed }}</span>
            </div>
          </div>
        </div>

        <!-- 状态条（最新日志，点击展开/收起） -->
        <div
          class="status-bar"
          :class="{ clickable: logEntries.length > 0, open: logExpanded }"
          :title="logExpanded ? '点击收起日志' : '点击展开日志'"
          @click="logEntries.length && (logExpanded = !logExpanded)"
        >
          <div class="status-left">
            <template v-if="currentOp">
              <span class="spinner" />
              <span class="status-text busy">{{ currentOp.text }}（{{ opElapsed }}）</span>
            </template>
            <template v-else>
              <span v-if="statusLevel !== 'info'" class="status-ico" :class="statusLevel">
                <LogIcon :level="statusLevel" />
              </span>
              <span class="status-text" :class="{ busy }">{{ statusText }}</span>
            </template>
          </div>
          <button v-if="busy" class="pill-btn danger cancel-btn" @click.stop="cancelJob">取消</button>
          <div class="status-right">
            <svg class="chev" width="10" height="10" viewBox="0 0 10 10" fill="none">
              <path v-if="logExpanded" d="M2.5 6.5L5 4l2.5 2.5" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" />
              <path v-else d="M2.5 3.5L5 6l2.5-2.5" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
            <span v-if="busy" class="job-timer">总耗时 {{ busyText }}</span>
            <span>{{ apps.length }} 个应用</span>
            <span>{{ sizeText(totalSizeKb) }}</span>
          </div>
        </div>
        </template>
      </section>
    </div>

    <!-- 右键菜单 -->
    <div
      v-if="menuOpen && menuApp"
      class="ctx-menu"
      :style="{ left: menuX + 'px', top: menuY + 'px' }"
      @click.stop
      @contextmenu.prevent.stop
    >
      <div class="ctx-app-name">{{ menuApp.name }}</div>
      <button class="ctx-item" :disabled="busy || !menuApp.uninstallString" @click="uninstall(menuApp)">
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
          <path d="M7 1.5v7M4.2 5.7L7 8.5l2.8-2.8" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" />
          <path d="M1.8 10.5h10.4" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
        </svg>
        <span>卸载</span>
      </button>
      <button class="ctx-item danger" :disabled="busy" @click="askForce(menuApp)">
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
          <path d="M2.5 4h9M5.5 4V2.8c0-.4.3-.8.8-.8h1.4c.5 0 .8.4.8.8V4M3.5 4l.6 7.2c0 .4.4.8.8.8h4.2c.4 0 .8-.4.8-.8L10.5 4" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" />
        </svg>
        <span>强制卸载</span>
      </button>
      <div class="ctx-sep" />
      <button class="ctx-item" :disabled="busy" @click="cleanRegistry(menuApp)">
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
          <rect x="2" y="1.8" width="10" height="10.4" rx="2" stroke="currentColor" stroke-width="1.2" />
          <path d="M4.5 5h5M4.5 7.3h5M4.5 9.6h3" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
        </svg>
        <span>清除注册表</span>
      </button>
      <button class="ctx-item" :disabled="busy" @click="cleanCache(menuApp)">
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
          <path d="M1.8 3.5c0-.7.6-1.2 1.2-1.2h2.4l1.2 1.4h4.4c.6 0 1.2.5 1.2 1.2v6c0 .7-.6 1.2-1.2 1.2H3c-.6 0-1.2-.5-1.2-1.2v-7.4z" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round" />
        </svg>
        <span>清除缓存文件夹</span>
      </button>
    </div>

    <!-- 强制卸载确认 -->
    <div v-if="pendingForce" class="overlay" @click.self="pendingForce = null">
      <div class="sheet" style="width: 400px">
        <h2>强制卸载「{{ pendingForce.name }}」？</h2>
        <p class="sheet-text">
          将结束其进程、<b>直接删除</b>安装目录与注册表键并清理残留。此操作不可恢复（数据目录与快捷方式会进回收站）。
        </p>
        <div class="sheet-actions">
          <button class="pill-btn" @click="pendingForce = null">取消</button>
          <button class="pill-btn danger" @click="confirmForce">强制卸载</button>
        </div>
      </div>
    </div>

    <!-- 操作报告已移除：结果由日志逐条展示 + toast 提示 -->

    <!-- toast -->
    <div v-if="toast" class="toast" :class="toast.kind">
      <span class="toast-icon">
        <svg v-if="toast.kind === 'ok'" width="14" height="14" viewBox="0 0 14 14" fill="none">
          <circle cx="7" cy="7" r="6" stroke="currentColor" stroke-width="1.3" />
          <path d="M4.4 7.2l1.8 1.8 3.4-3.8" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" />
        </svg>
        <svg v-else width="14" height="14" viewBox="0 0 14 14" fill="none">
          <circle cx="7" cy="7" r="6" stroke="currentColor" stroke-width="1.3" />
          <path d="M7 4v3.4M7 9.8v.2" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" />
        </svg>
      </span>
      {{ toast.text }}
    </div>
  </div>
</template>
