<script lang="ts">
  import Tab from "../lib/Tab.svelte";

  // import { invoke } from "@tauri-apps/api/core";

  let tabs = $state<string[]>([]);
  let activeTab = $state(0);
</script>

<!-- 
<div class="flex flex-wrap">
  {#each tabs as tab}
    <button class="px-2 py-1 border border-amber-500 cursor-pointer">
      {tab}
    </button>
  {/each}
  <button
    onclick={() => {
      tabs.push("Undefined");
    }}
    class="px-2 py-1 border border-amber-500 cursor-pointer">+</button
  >
</div> -->
