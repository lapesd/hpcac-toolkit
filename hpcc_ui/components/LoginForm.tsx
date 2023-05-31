interface LoginProps {
  isAllowed: boolean;
}

function LoginButton() {
  const loginButtonStyle =
    "border border-black bg-white text-black hover:bg-black hover:text-white px-4 py-2 mt-4 transition-colors duration-200";

  return <button className={loginButtonStyle} type="submit">Submit</button>;
}

export default function LoginForm({ isAllowed }: LoginProps) {
  const loginPositioning = "flex items-center justify-center h-screen";
  const loginFormStyle =
    "border border-black bg-white shadow-md p-8 flex flex-col w-64";
  const inputFieldStyle = "w-full border border-black bg-white px-2 py-1 mt-2";

  return (
    <div className={loginPositioning}>
      <div className={loginFormStyle}>
        <p>You currently {isAllowed ? "are" : "are not"} logged in.</p>
        <form method="post" action="/api/login">
          <input className={inputFieldStyle} type="text" name="username" />
          <input className={inputFieldStyle} type="password" name="password" />
          <LoginButton />
        </form>
      </div>
    </div>
  );
}
