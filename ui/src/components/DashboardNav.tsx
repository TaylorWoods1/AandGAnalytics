import { Link } from 'react-router-dom';

export type NavPage =
  | 'home'
  | 'flow'
  | 'sprints'
  | 'epics'
  | 'explore'
  | 'ask'
  | 'sync'
  | 'settings';

const LINKS: { id: NavPage; to: string; label: string }[] = [
  { id: 'home', to: '/', label: 'Home' },
  { id: 'flow', to: '/flow', label: 'Flow' },
  { id: 'sprints', to: '/sprints', label: 'Sprints' },
  { id: 'epics', to: '/epics', label: 'Epics' },
  { id: 'explore', to: '/explore', label: 'Explore' },
  { id: 'ask', to: '/ask', label: 'Ask AI' },
  { id: 'sync', to: '/sync', label: 'Sync' },
  { id: 'settings', to: '/settings', label: 'Settings' },
];

export default function DashboardNav({ current }: { current: NavPage }) {
  return (
    <nav className="dashboard-nav">
      {LINKS.map((link) => (
        <Link
          key={link.id}
          to={link.to}
          {...(link.id === current ? { 'aria-current': 'page' as const } : {})}
        >
          {link.label}
        </Link>
      ))}
    </nav>
  );
}
