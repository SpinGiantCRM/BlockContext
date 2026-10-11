import { readFile } from 'node:fs/promises';
import { AppError } from './types.js';
import { uuidValue } from './upstream.js';
export const DEMO_UUID = '0123456789abcdef0123456789abcdef';
// All identities and profile values in demo mode are synthetic fixtures.
export class DemoUpstream {
    source = 'synthetic_demo';
    async resolve(player) {
        if (player.toLowerCase() !== 'demoplayer' && uuidValue(player) !== DEMO_UUID)
            throw new AppError('DEMO_PLAYER_ONLY', 'Demo mode only includes a synthetic DemoPlayer profile.');
        return { uuid: DEMO_UUID, name: 'DemoPlayer' };
    }
    async profiles(uuid) {
        if (uuid !== DEMO_UUID)
            throw new AppError('DEMO_PLAYER_ONLY', 'No fixture exists for that UUID.');
        const fixture = JSON.parse(await readFile(new URL('../../fixtures/synthetic-profile.json', import.meta.url), 'utf8'));
        return fixture.profiles;
    }
    async skillCatalog() {
        return { skills: Object.fromEntries(['COMBAT', 'MINING', 'FARMING', 'HUNTING'].map(s => [s, { levels: [{ level: 1, totalExpRequired: 50 }, { level: 2, totalExpRequired: 175 }, { level: 3, totalExpRequired: 375 }] }])) };
    }
    async itemCatalog() { return { items: {} }; }
    async playerMetadata() { return { newPackageRank: 'NONE', rank: 'NORMAL', packageRank: 'NONE', monthlyPackageRank: 'NONE' }; }
    async garden() { return { garden_experience: 10, resources_collected: { WHEAT: 73 }, crop_upgrade_levels: {}, garden_upgrades: {}, unlocked_plots_ids: ['beginner_2'], commission_data: { visits: {}, completed: {} } }; }
    async bazaar() { return { lastUpdated: Date.now(), products: { WHEAT: { buy_summary: [{ amount: 10, pricePerUnit: 8, orders: 1 }], sell_summary: [{ amount: 10, pricePerUnit: 3, orders: 1 }], quick_status: { buyPrice: 8, sellPrice: 3, buyVolume: 10, sellVolume: 10 } } } }; }
    async auctionPage(_page, _signal) { return { page: 0, totalPages: 1, totalAuctions: 0, lastUpdated: Date.now(), auctions: [] }; }
}
