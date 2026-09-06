import { describe, expect, it, vi } from 'vitest'

import type { RequestContext } from '../../types/request'
import { OfflineCacheFeature } from './offline-cache'

function makeContext(
	path = '/search',
	params?: Record<string, unknown>,
	method: RequestContext['options']['method'] = 'GET',
): RequestContext {
	return {
		url: `https://api.modrinth.com/v2${path}`,
		path,
		options: { api: 'labrinth', version: 2, params, method },
		attempt: 1,
		startTime: 0,
	}
}

describe('OfflineCacheFeature', () => {
	it('stores successful GET responses under a canonicalized key', async () => {
		const set = vi.fn()
		const feature = new OfflineCacheFeature({ get: vi.fn(), set, isOffline: () => false })

		const result = await feature.execute(
			() => Promise.resolve({ hits: ['a'] }),
			makeContext('/search', { q: 'test', limit: 20 }),
		)

		expect(result).toEqual({ hits: ['a'] })
		expect(set).toHaveBeenCalledWith('search', 'search?limit=20&q=test', { hits: ['a'] })
	})

	it('serves the cached body when offline and the request fails', async () => {
		const cached = { hits: ['cached'] }
		const get = vi.fn().mockResolvedValue(cached)
		const feature = new OfflineCacheFeature({ get, set: vi.fn(), isOffline: () => true })

		const result = await feature.execute(
			() => Promise.reject(new Error('network')),
			makeContext('/search'),
		)

		expect(result).toEqual(cached)
		expect(get).toHaveBeenCalledWith('search', 'search')
	})

	it('rethrows the original error when offline and no cache exists', async () => {
		const feature = new OfflineCacheFeature({
			get: vi.fn().mockResolvedValue(undefined),
			set: vi.fn(),
			isOffline: () => true,
		})

		await expect(
			feature.execute(() => Promise.reject(new Error('network')), makeContext('/search')),
		).rejects.toThrow('network')
	})

	it('does not cache non-GET requests', async () => {
		const set = vi.fn()
		const feature = new OfflineCacheFeature({ get: vi.fn(), set, isOffline: () => false })

		await feature.execute(
			() => Promise.resolve({ ok: true }),
			makeContext('/project/sodium', undefined, 'POST'),
		)

		expect(set).not.toHaveBeenCalled()
	})
})
