<script setup lang="ts">
import { ref, onMounted, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";

interface AppUsage {
  app_name: string;
  duration: number;
  exe_path: string;
  icon_base64?: string;
}

const stats = ref<AppUsage[]>([]);
const totalDuration = ref<number>(0);
let intervalId: number | undefined;

async function fetchIconsForStats(data: AppUsage[]) {
  for (const item of data) {
    if (item.exe_path && !item.icon_base64) {
      try {
        const base64 = await invoke<string>("get_app_icon_native", { exePath: item.exe_path });
        if (base64) {
          item.icon_base64 = `data:image/png;base64,${base64}`;
        }
      } catch (e) {
        console.warn(`Failed to fetch icon for ${item.app_name}`, e);
      }
    }
  }
}

async function checkWindowUrl() {
  try {
    const url = await invoke<string>("check_window_url");
    console.log("Current URL:", url);
  } catch (error) {
    console.error("Failed to check window URL", error);
  }
}

async function fetchStats() {
  try {
    const data = await invoke<AppUsage[]>("get_today_stats");
    
    // Preserve existing icons to avoid flickering
    const currentStats = stats.value;
    data.forEach(newItem => {
      const existing = currentStats.find(oldItem => oldItem.app_name === newItem.app_name);
      if (existing && existing.icon_base64) {
        newItem.icon_base64 = existing.icon_base64;
      }
    });

    stats.value = data;
    totalDuration.value = data.reduce((acc, curr) => acc + curr.duration, 0);
    
    // Fetch new icons in the background
    fetchIconsForStats(data);
  } catch (error) {
    console.error("Failed to fetch stats", error);
  }
}

onMounted(() => {
  fetchStats();
  // Fetch stats every 5 seconds for near real-time updates
  intervalId = window.setInterval(fetchStats, 5000);
});

onUnmounted(() => {
  if (intervalId !== undefined) {
    clearInterval(intervalId);
  }
});

function formatDuration(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = seconds % 60;
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return `${m}m ${s}s`;
  return `${s}s`;
}
</script>

<template>
  <main class="min-h-screen bg-linear-to-br from-slate-50 via-blue-50/30 to-indigo-50/50 p-6 sm:p-8 text-gray-800">
    <div class="max-w-4xl mx-auto">
      <header class="mb-8 flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4 sm:gap-0">
        <div>
          <h1 class="text-4xl font-bold bg-linear-to-r from-blue-600 to-indigo-600 bg-clip-text text-transparent">rsiew</h1>
          <p class="text-gray-500 mt-1">Personal Work Dynamics Monitor</p>
        </div>
        <div class="bg-linear-to-br from-blue-500 to-indigo-600 px-6 py-5 rounded-2xl shadow-lg shadow-blue-500/25 text-center min-w-[160px]">
          <p class="text-xs text-blue-100 uppercase font-semibold tracking-wider">Today's Total</p>
          <p class="text-3xl font-bold text-white mt-1">{{ formatDuration(totalDuration) }}</p>
        </div>
        <button class="bg-linear-to-r from-blue-500 to-indigo-600 px-6 py-2 rounded-md text-white font-bold" @click="checkWindowUrl">Check URL</button>
      </header>

      <div class="bg-white/80 backdrop-blur-sm rounded-2xl shadow-xl shadow-gray-200/50 border border-white/50 overflow-hidden">
        <div class="px-6 py-5 border-b border-gray-100/80 bg-linear-to-r from-gray-50/80 to-blue-50/50 flex justify-between items-center">
          <h2 class="text-lg font-semibold text-gray-800 flex items-center gap-2">
            <svg class="w-5 h-5 text-blue-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z"></path>
            </svg>
            App Usage
          </h2>
          <div class="flex items-center text-xs text-green-600 bg-green-50 px-3 py-1.5 rounded-full">
            <span class="relative flex h-2 w-2 mr-2">
              <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-green-400 opacity-75"></span>
              <span class="relative inline-flex rounded-full h-2 w-2 bg-green-500"></span>
            </span>
            Live Updates
          </div>
        </div>

        <div class="p-0">
          <ul class="divide-y divide-gray-100/80">
            <li v-for="(item, index) in stats" :key="index" class="px-6 py-4 hover:bg-linear-to-r hover:from-blue-50/40 hover:to-indigo-50/40 transition-all duration-200 group">
              <div class="flex items-center justify-between mb-2">
                <div class="flex items-center space-x-4">
                  <div v-if="item.icon_base64" class="w-12 h-12 shrink-0 flex items-center justify-center rounded-xl bg-gray-50 shadow-sm border border-gray-100 group-hover:border-blue-200 group-hover:shadow-md transition-all">
                    <img :src="item.icon_base64" class="w-full h-full object-contain p-1.5" :alt="item.app_name" />
                  </div>
                  <div v-else class="w-12 h-12 rounded-xl bg-gradient-to-br from-blue-500 to-indigo-600 text-white flex items-center justify-center font-bold text-xl shadow-lg shadow-blue-500/30 group-hover:scale-105 transition-transform">
                    {{ item.app_name.charAt(0).toUpperCase() }}
                  </div>
                  <div>
                    <h3 class="font-semibold text-gray-900 group-hover:text-blue-600 transition-colors">{{ item.app_name }}</h3>
                    <p class="text-sm text-gray-500 mt-0.5">
                      {{ totalDuration > 0 ? ((item.duration / totalDuration) * 100).toFixed(1) : 0 }}% of total
                    </p>
                  </div>
                </div>
                <div class="text-right">
                  <p class="text-lg font-bold text-gray-900">{{ formatDuration(item.duration) }}</p>
                </div>
              </div>
              <div class="ml-16">
                <div class="h-2 bg-gray-100 rounded-full overflow-hidden">
                  <div
                    class="h-full bg-gradient-to-r from-blue-500 to-indigo-500 rounded-full transition-all duration-500 ease-out"
                    :style="{ width: totalDuration > 0 ? `${(item.duration / totalDuration) * 100}%` : '0%' }"
                  ></div>
                </div>
              </div>
            </li>

            <li v-if="stats.length === 0" class="px-6 py-16 text-center">
              <div class="inline-flex items-center justify-center w-16 h-16 rounded-full bg-gray-100 mb-4">
                <svg class="w-8 h-8 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z"></path>
                </svg>
              </div>
              <p class="text-gray-500 font-medium">No activity recorded yet today.</p>
              <p class="text-sm text-gray-400 mt-1">Start working and check back here!</p>
            </li>
          </ul>
        </div>
      </div>
      <footer class="mt-6 text-center text-sm text-gray-400">
        <p>Monitoring your work dynamics • Updates every 5 seconds</p>
      </footer>
    </div>
  </main>
</template>