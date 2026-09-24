import { invoke } from '@tauri-apps/api/core'
import { ref } from 'vue'

const isOffline = ref(typeof navigator !== 'undefined' && !navigator.onLine)
const onlineHandlers: Array<() => void> = []

export function useConnectivity() {
	async function check(): Promise<boolean> {
		const offline = await invoke<boolean>('plugin:connectivity|check')
		isOffline.value = offline
		return offline
	}

	function setOffline(offline: boolean): void {
		isOffline.value = offline
		void invoke('plugin:connectivity|set_offline', { offline }).catch(() => {})
	}

	function registerOnlineHandler(handler: () => void): void {
		onlineHandlers.push(handler)
	}

	return { isOffline, check, setOffline, registerOnlineHandler }
}

if (typeof window !== 'undefined') {
	window.addEventListener('online', () => {
		isOffline.value = false
		void invoke('plugin:connectivity|set_offline', { offline: false }).catch(() => {})
		onlineHandlers.forEach((handler) => handler())
	})
	window.addEventListener('offline', () => {
		isOffline.value = true
		void invoke('plugin:connectivity|set_offline', { offline: true }).catch(() => {})
	})
}
