import cityCodes from '../../sidecar/vendor/city_codes.json';

export const POPULAR_BOSS_CITIES = [
  '北京', '上海', '广州', '深圳', '杭州', '天津', '西安', '苏州', '武汉', '厦门', '长沙', '成都', '郑州',
  '重庆', '佛山', '合肥', '济南', '青岛', '南京', '东莞', '昆明', '南昌', '石家庄', '宁波', '福州'
] as const;

export const ALL_BOSS_CITIES: readonly string[] = Object.freeze(Object.keys(cityCodes));

const cityNames = new Set(ALL_BOSS_CITIES);

export function isBossCityName(value: string): boolean {
  return cityNames.has(value.trim());
}

export function matchingBossCities(query: string): readonly string[] {
  const normalized = query.trim();
  return normalized
    ? ALL_BOSS_CITIES.filter((city) => city.includes(normalized))
    : POPULAR_BOSS_CITIES;
}
