export default function Dashboard() {
  const dashboardStyle = "bg-white flex items-center justify-center h-screen";

  return (
    <div className={dashboardStyle}>
      <a href="/api/logout">Logout</a>
    </div>
  );
}
