import { beforeEach, describe, expect, it, vi } from 'vitest'

import { useConnectivity } from './useConnectivity'

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }))

vi.mock('@tauri-apps/api/core', () => ({ invoke }))

describe('useConnectivity', () => {
	beforeEach(() => {
		invoke.mockReset()
		invoke.mockResolvedValue(undefined)
	})

	it('check() reads the offline state from the backend and exposes it', async () => {
		invoke.mockResolvedValueOnce(true)
		const connectivity = useConnectivity()
		expect(await connectivity.check()).toBe(true)
		expect(connectivity.isOffline.value).toBe(true)
		expect(invoke).toHaveBeenCalledWith('plugin:connectivity|check')
	})

	it('check() updates the state when connectivity is restored', async () => {
		invoke.mockResolvedValueOnce(false)
		const connectivity = useConnectivity()
		expect(await connectivity.check()).toBe(false)
		expect(connectivity.isOffline.value).toBe(false)
	})

	it('setOffline() updates the flag and informs the backend', async () => {
		const connectivity = useConnectivity()
		connectivity.setOffline(true)
		expect(connectivity.isOffline.value).toBe(true)
		expect(invoke).toHaveBeenCalledWith('plugin:connectivity|set_offline', { offline: true })
	})
})
