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
  <main class="min-h-screen bg-gray-50 p-8 text-gray-800">
    <div class="max-w-4xl mx-auto">
      <header class="mb-8 flex items-center justify-between">
        <div>
          <h1 class="text-3xl font-bold text-gray-900">rsiew</h1>
          <p class="text-gray-500 mt-2">Personal Work Dynamics Monitor</p>
        </div>
        <div class="bg-white px-6 py-4 rounded-xl shadow-sm border border-gray-100 text-center">
          <p class="text-sm text-gray-500 uppercase font-semibold tracking-wider">Today's Total</p>
          <p class="text-3xl font-bold text-blue-600 mt-1">{{ formatDuration(totalDuration) }}</p>
        </div>
      </header>

      <div class="bg-white rounded-xl shadow-sm border border-gray-100 overflow-hidden">
        <div class="px-6 py-5 border-b border-gray-100 bg-gray-50/50 flex justify-between items-center">
          <h2 class="text-lg font-semibold text-gray-800">App Usage</h2>
          <div class="flex items-center text-xs text-gray-400">
            <span class="relative flex h-2 w-2 mr-2">
              <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-green-400 opacity-75"></span>
              <span class="relative inline-flex rounded-full h-2 w-2 bg-green-500"></span>
            </span>
            Live Updates
          </div>
        </div>
        
        <div class="p-0">
          <ul class="divide-y divide-gray-100">
            <li v-for="(item, index) in stats" :key="index" class="px-6 py-4 hover:bg-gray-50 transition-colors flex items-center justify-between group">
              <div class="flex items-center space-x-4">
                <div v-if="item.icon_base64" class="w-10 h-10 shrink-0 flex items-center justify-center">
                  <img :src="item.icon_base64" class="w-full h-full object-contain" :alt="item.app_name" />
                </div>
                <div v-else class="w-10 h-10 rounded-lg bg-blue-100 text-blue-600 flex items-center justify-center font-bold text-lg">
                  {{ item.app_name.charAt(0).toUpperCase() }}
                </div>
                <div>
                  <h3 class="font-medium text-gray-900">{{ item.app_name }}</h3>
                  <p class="text-sm text-gray-500">
                    {{ totalDuration > 0 ? ((item.duration / totalDuration) * 100).toFixed(1) : 0 }}% of total time
                  </p>
                </div>
              </div>
              <div class="text-right">
                <p class="font-semibold text-gray-900">{{ formatDuration(item.duration) }}</p>
              </div>
            </li>
            
            <li v-if="stats.length === 0" class="px-6 py-12 text-center text-gray-500">
              <p>No activity recorded yet today.</p>
            </li>
          </ul>
        </div>
      </div>
    </div>
  </main>
</template>