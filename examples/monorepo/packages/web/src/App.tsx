type AppProps = {
  name: string;
};

export function App({ name }: AppProps) {
  return <main>Hello, {name}</main>;
}
