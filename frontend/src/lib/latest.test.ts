import { describe, expect, it } from 'vitest';

import { latest } from './latest';

function deferred<T>() {
	let resolve!: (value: T) => void;
	let reject!: (reason?: unknown) => void;
	const promise = new Promise<T>((res, rej) => {
		resolve = res;
		reject = rej;
	});
	return { promise, resolve, reject };
}

const flush = () => new Promise((resolve) => setTimeout(resolve));

describe('latest', () => {
	it('applies a result in the plain case', async () => {
		const applied: string[] = [];
		const search = latest(
			(value: string) => Promise.resolve(value),
			(v) => applied.push(v),
		);

		search('one');
		await flush();

		expect(applied).toEqual(['one']);
	});

	it('drops a stale response that arrives after a newer one', async () => {
		const calls: ReturnType<typeof deferred<string>>[] = [];
		const applied: string[] = [];
		const search = latest(
			() => {
				const call = deferred<string>();
				calls.push(call);
				return call.promise;
			},
			(v) => applied.push(v),
		);

		search();
		search();
		calls[1].resolve('newer');
		await flush();
		calls[0].resolve('stale');
		await flush();

		expect(applied).toEqual(['newer']);
	});

	it('drops a response overtaken by a call that started after it resolved elsewhere', async () => {
		const calls: ReturnType<typeof deferred<string>>[] = [];
		const applied: string[] = [];
		const search = latest(
			() => {
				const call = deferred<string>();
				calls.push(call);
				return call.promise;
			},
			(v) => applied.push(v),
		);

		search();
		search();
		calls[0].resolve('stale');
		await flush();
		calls[1].resolve('newer');
		await flush();

		expect(applied).toEqual(['newer']);
	});

	it('drops whatever is in flight after cancel', async () => {
		const call = deferred<string>();
		const applied: string[] = [];
		const search = latest(
			() => call.promise,
			(v) => applied.push(v),
		);

		search();
		search.cancel();
		call.resolve('cancelled');
		await flush();

		expect(applied).toEqual([]);
	});

	it('swallows failures', async () => {
		const call = deferred<string>();
		const applied: string[] = [];
		const search = latest(
			() => call.promise,
			(v) => applied.push(v),
		);

		search();
		call.reject(new Error('network'));
		await flush();

		expect(applied).toEqual([]);
	});
});
