import { invoke } from '@tauri-apps/api/core'

export async function queue_enqueue(kind, state) {
	return await invoke('plugin:queue|enqueue', { kind, state })
}

export async function queue_list() {
	return await invoke('plugin:queue|list')
}

export async function queue_remove(id) {
	return await invoke('plugin:queue|remove', { id })
}
