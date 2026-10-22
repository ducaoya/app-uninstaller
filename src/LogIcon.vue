<script setup lang="ts">
import type { LogLine } from "./types";

defineProps<{ level: LogLine["level"] }>();
</script>

<template>
  <span v-if="level === 'ok'" class="lico">
    <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
      <circle cx="6" cy="6" r="5" stroke="currentColor" stroke-width="1.2" />
      <path d="M3.8 6.2l1.5 1.5 2.9-3.2" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" />
    </svg>
  </span>
  <span v-else-if="level === 'err'" class="lico">
    <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
      <circle cx="6" cy="6" r="5" stroke="currentColor" stroke-width="1.2" />
      <path d="M4.2 4.2l3.6 3.6M7.8 4.2L4.2 7.8" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
    </svg>
  </span>
  <span v-else-if="level === 'warn'" class="lico">
    <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
      <path d="M6 1.6l4.8 8.6H1.2L6 1.6z" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round" />
      <path d="M6 4.6v2.2M6 8.4v.2" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
    </svg>
  </span>
  <span v-else-if="level === 'phase'" class="spinner tiny" />
</template>

<style scoped>
.lico {
  display: inline-flex;
  flex: none;
}
</style>
