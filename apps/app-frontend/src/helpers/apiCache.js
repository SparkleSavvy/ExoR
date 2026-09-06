import { invoke } from '@tauri-apps/api/core'

export async function api_cache_get(namespace, key) {
	return await invoke('plugin:api-cache|get_cached', { namespace, key })
}

export async function api_cache_set(namespace, key, data) {
	return await invoke('plugin:api-cache|set_cached', { namespace, key, data })
}

export async function api_cache_delete(namespace, key) {
	return await invoke('plugin:api-cache|delete_cached', { namespace, key })
}

export async function api_cache_clear(namespace = null) {
	return await invoke('plugin:api-cache|clear_cached', { namespace })
}

export async function api_cache_keys(namespace) {
	return await invoke('plugin:api-cache|keys_cached', { namespace })
}
