import { AbstractFeature, type FeatureConfig } from '../core/abstract-feature'
import type { RequestContext } from '../types/request'

/**
 * Offline cache feature configuration
 */
export interface OfflineCacheConfig extends FeatureConfig {
	/**
	 * Load a cached body from persistent storage
	 */
	get: (namespace: string, key: string) => Promise<unknown>

	/**
	 * Store a body into persistent storage
	 */
	set: (namespace: string, key: string, data: unknown) => Promise<unknown>

	/**
	 * Whether the app is currently offline.
	 * When true, failed requests fall back to the cached response.
	 */
	isOffline: () => boolean

	/**
	 * Optional custom namespace resolver.
	 * Return undefined to skip caching for a request.
	 * @default derives the namespace from the first path segment
	 */
	namespaceFor?: (url: string, path: string) => string | undefined
}

const DEFAULT_NAMESPACES: Record<string, string> = {
	search: 'search',
	project: 'projects',
	projects: 'projects',
	version: 'versions',
	versions: 'versions',
	tag: 'tags',
	tags: 'tags',
	team: 'teams',
	user: 'users',
	users: 'users',
	notification: 'notifications',
	report: 'reports',
}

/**
 * Offline cache feature
 *
 * Stores successful GET responses and serves them back when a request
 * fails while offline, so previously-loaded pages keep rendering.
 * Only caches GET requests; POST/PATCH/DELETE and binary downloads are ignored.
 */
export class OfflineCacheFeature extends AbstractFeature {
	declare protected config: Required<OfflineCacheConfig>

	private mirror = new Map<string, Map<string, unknown>>()

	constructor(config: OfflineCacheConfig) {
		super(config)

		this.config = {
			enabled: true,
			name: 'offline-cache',
			...config,
		} as Required<OfflineCacheConfig>
	}

	async execute<T>(next: () => Promise<T>, context: RequestContext): Promise<T> {
		if ((context.options.method ?? 'GET') !== 'GET') {
			return next()
		}

		if (context.metadata?.isUpload) {
			return next()
		}

		const namespace = this.resolveNamespace(context.url, context.path)
		if (!namespace) {
			return next()
		}

		const key = this.buildKey(context)

		try {
			const result = await next()

			this.cache(result, namespace, key)

			return result
		} catch (error) {
			if (this.config.isOffline()) {
				const cached = await this.retrieve(namespace, key)
				if (cached !== undefined && cached !== null) {
					return cached as T
				}
			}

			throw error
		}
	}

	/**
	 * Compute the cache key for a request: path + canonicalized (sorted) query params
	 */
	private buildKey(context: RequestContext): string {
		const path = context.path.replace(/^\//, '')

		const params = context.options.params
		if (!params || Object.keys(params).length === 0) {
			return path
		}

		const canonical = Object.keys(params)
			.sort()
			.map((key) => `${key}=${this.encodeParam(params[key])}`)
			.join('&')

		return `${path}?${canonical}`
	}

	private encodeParam(value: unknown): string {
		return encodeURIComponent(Array.isArray(value) ? value.join(',') : String(value))
	}

	private resolveNamespace(url: string, path: string): string | undefined {
		if (this.config.namespaceFor) {
			return this.config.namespaceFor(url, path)
		}

		if (path.includes('/fs/download')) {
			return undefined
		}

		const segment = path.split('?')[0].split('/').filter(Boolean)[0]
		if (!segment) {
			return undefined
		}

		return DEFAULT_NAMESPACES[segment] ?? segment
	}

	private cache(data: unknown, namespace: string, key: string): void {
		if (data === undefined || data === null) {
			return
		}

		if (typeof Blob !== 'undefined' && data instanceof Blob) {
			return
		}

		this.setMirror(namespace, key, data)

		// Persistence is best-effort and must never break the response.
		void Promise.resolve(this.config.set(namespace, key, data)).catch(() => {})
	}

	private async retrieve(namespace: string, key: string): Promise<unknown> {
		const entry = this.mirror.get(namespace)?.get(key)
		if (entry !== undefined) {
			return entry
		}

		const stored = await Promise.resolve(this.config.get(namespace, key))
		if (stored !== undefined && stored !== null) {
			this.setMirror(namespace, key, stored)
		}

		return stored
	}

	private setMirror(namespace: string, key: string, data: unknown): void {
		let nsMap = this.mirror.get(namespace)
		if (!nsMap) {
			nsMap = new Map()
			this.mirror.set(namespace, nsMap)
		}
		nsMap.set(key, data)
	}
}
