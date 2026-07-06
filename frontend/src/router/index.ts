import { createRouter, createWebHashHistory } from 'vue-router'

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    {
      path: '/',
      name: 'home',
      component: () => import('../views/HomeView.vue'),
    },
    {
      path: '/reader',
      name: 'reader',
      component: () => import('../views/ReaderView.vue'),
    },
    {
      path: '/ai-book',
      name: 'ai-book',
      component: () => import('../views/AiBookV4View.vue'),
    },
    {
      path: '/explore',
      name: 'explore',
      component: () => import('../views/ExploreView.vue'),
    },
    {
      path: '/recent',
      name: 'recent',
      component: () => import('../views/RecentView.vue'),
    },
    {
      path: '/rss',
      name: 'rss',
      component: () => import('../views/RssView.vue'),
    },
    {
      path: '/rss/manage',
      name: 'rss-manage',
      component: () => import('../views/RssManageView.vue'),
    },
    {
      path: '/rss/article',
      name: 'rss-article',
      component: () => import('../views/RssArticleView.vue'),
    },
  ],
})

export default router
