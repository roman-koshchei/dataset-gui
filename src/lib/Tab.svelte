<script lang="ts">
    let tabs = $state<{ id: number; title: string; color: string }[]>([]);
    let activeTab = $state(0);
    let nextId = $state(1);

    let dataSets = $state<{ id: string; tabId: number; imagesDir: string; labelsDir: string }[]>([]);
    let imagesDir = $state('');
    let labelsDir = $state('');

    const colors = [
        'bg-red-600', 'bg-blue-600', 'bg-green-600', 'bg-yellow-600',
        'bg-purple-600', 'bg-pink-600', 'bg-indigo-600', 'bg-teal-600',
        'bg-orange-600', 'bg-emerald-600', 'bg-cyan-600', 'bg-violet-600',
        'bg-rose-600', 'bg-amber-600', 'bg-lime-600', 'bg-sky-600'
    ];

    function addNewTab() {
        const randomColor = colors[Math.floor(Math.random() * colors.length)];
        tabs.push({
            id: nextId,
            title: `Tab ${nextId}`,
            color: randomColor
        });
        activeTab = tabs.length - 1;
        nextId++;
    }

    function addNewDataset() {
        dataSets.push({
            id: crypto.randomUUID(),
            tabId: tabs[activeTab]?.id || 0,
            imagesDir: imagesDir,
            labelsDir: labelsDir
        });
        imagesDir = '';
        labelsDir = '';
    }

    function setActiveTab(index: number) {
        activeTab = index;
    }

    function closeTab(index: number, event: Event) {
        event.stopPropagation();
        tabs.splice(index, 1);
        if (activeTab >= tabs.length && tabs.length > 0) {
            activeTab = tabs.length - 1;
        } else if (tabs.length === 0) {
            activeTab = 0;
        } else if (activeTab > index) {
            activeTab--;
        }
    }
</script>

<div class="grid grid-rows-[auto_1fr] h-screen w-screen">
    <div class="border-b border-gray-700 bg-dark overflow-x-auto scrollbar-thin scrollbar-thumb-gray-700 scrollbar-track-gray-800">
        <div class="flex items-center">
            {#each tabs as tab, index}
                <div
                        class="flex-shrink-0 relative border-r border-gray-800 transition-colors duration-150
                {activeTab === index
                  ? 'bg-dark border-b-2 border-b-blue-500 text-blue-200'
                  : 'hover:bg-gray-800 text-gray-200'
                }"
                >
                    <div class="flex items-center">
                        <button
                                class="px-4 py-2 cursor-pointer flex-1"
                                onclick={() => setActiveTab(index)}
                        >
                            <span class="whitespace-nowrap">{tab.title}</span>
                        </button>
                        <button
                                class="hover:bg-red-900 rounded p-1 mr-1 transition-opacity duration-150"
                                onclick={(event) => closeTab(index, event)}
                                title="Close tab"
                        >
                            <svg class="w-3 h-3" fill="currentColor" viewBox="0 0 20 20">
                                <path fill-rule="evenodd" d="M4.293 4.293a1 1 0 011.414 0L10 8.586l4.293-4.293a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 01-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 01-1.414-1.414L8.586 10 4.293 5.707a1 1 0 010-1.414z" clip-rule="evenodd"></path>
                            </svg>
                        </button>
                    </div>
                </div>
            {/each}

            <button
                    class="flex-shrink-0 px-3 py-2 text-gray-400 hover:text-gray-200 hover:bg-gray-800 transition-colors duration-150"
                    onclick={addNewTab}
                    title="Add new tab"
            >
                <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4"></path>
                </svg>
            </button>
        </div>
    </div>


    <div class="overflow-auto">
        {#each tabs as tab, index}
            <div
                class="h-full w-full {tab.color} p-6 {activeTab === index ? 'block' : 'hidden'}"
            >
                <div class="grid grid-cols-2 gap-8 h-full">
                    <div class="space-y-6">
                        <h3 class="text-lg font-medium text-gray-200">Add new dataset</h3>

                        <div class="space-y-4">
                            <div>
                                <label class="block text-sm font-medium text-gray-300 mb-2">Images directory</label>
                                <input
                                    type="text"
                                    class="w-full px-3 py-2 border border-gray-700 focus:outline-none focus:ring-2 focus:ring-gray-700 focus:border-transparent bg-gray-800 text-gray-100"
                                    placeholder="Select images folder..."
                                    bind:value={imagesDir}
                                />
                            </div>

                            <div>
                                <label class="block text-sm font-medium text-gray-300 mb-2">Labels directory</label>
                                <input
                                    type="text"
                                    class="w-full px-3 py-2 border border-gray-700 focus:outline-none focus:ring-2 focus:ring-gray-700 focus:border-transparent bg-gray-800 text-gray-100"
                                    placeholder="Select labels folder..."
                                    bind:value={labelsDir}
                                />
                            </div>

                            <div>
                                <button
                                    class="w-full bg-gray-700 text-gray-100 py-2 px-4 hover:bg-gray-800 transition-colors"
                                    onclick={addNewDataset}
                                >
                                    Add Dataset
                                </button>
                            </div>
                        </div>
                    </div>

                    <div class="space-y-6 overflow-y-auto">
                        <h3 class="text-lg font-medium text-gray-200">Select from history</h3>

                        <div class="space-y-3">
                            {#each dataSets.filter(ds => ds.tabId === tabs[index].id) as dataSet}
                                <button class="w-full p-3 border border-gray-700 hover:bg-gray-800 transition-colors text-left bg-gray-900">
                                    <div class="text-sm">
                                        <div class="font-medium text-gray-200">{dataSet.imagesDir}</div>
                                        <div class="font-medium text-gray-400">{dataSet.labelsDir}</div>
                                    </div>
                                </button>
                            {/each}
                        </div>
                    </div>
                </div>
            </div>
        {/each}
    </div>
</div>

<style>
    .scrollbar-thin::-webkit-scrollbar {
        height: 6px;
    }

    .scrollbar-thumb-gray-300::-webkit-scrollbar-thumb {
        background-color: #d1d5db;
        border-radius: 3px;
    }

    .scrollbar-track-gray-100::-webkit-scrollbar-track {
        background-color: #f3f4f6;
    }

    .scrollbar-thin {
        scrollbar-width: thin;
        scrollbar-color: #d1d5db #f3f4f6;
    }
</style>
