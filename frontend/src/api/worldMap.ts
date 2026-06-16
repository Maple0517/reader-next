import http from './http'
import type {
  WorldMapSpec,
  BuildWorldMapRequest,
  UpdateWorldMapRequest,
  UpdateWorldMapResponse,
  GenerateCoordinatesRequest,
  WorldMapCoordinates,
  WorldMapReviewItem,
  ResolveReviewRequest
} from '../types/worldMap'

// 获取世界地图规格书
export function getWorldMapSpec(bookUrl: string) {
  return http.get<WorldMapSpec | null>('/worldMap', {
    params: { book_url: bookUrl }
  }).then((r) => r.data)
}

// 构建世界地图规格书
export function buildWorldMap(data: BuildWorldMapRequest) {
  return http.post<WorldMapSpec>('/worldMap/build', data).then((r) => r.data)
}

// 保存世界地图规格书
export function saveWorldMapSpec(spec: WorldMapSpec) {
  return http.post<WorldMapSpec>('/worldMap/save', spec).then((r) => r.data)
}

// 增量更新世界地图
export function updateWorldMap(data: UpdateWorldMapRequest) {
  return http.post<UpdateWorldMapResponse>('/worldMap/update', data).then((r) => r.data)
}

// 生成坐标
export function generateCoordinates(data: GenerateCoordinatesRequest) {
  return http.post<WorldMapCoordinates>('/worldMap/generateCoordinates', data).then((r) => r.data)
}

// 获取审查清单
export function getReviewItems(bookUrl: string) {
  return http.get<WorldMapReviewItem[]>('/worldMap/reviewItems', {
    params: { book_url: bookUrl }
  }).then((r) => r.data)
}

// 人工修正审查项
export function resolveReviewItem(data: ResolveReviewRequest) {
  return http.post<string>('/worldMap/resolve', data).then((r) => r.data)
}
