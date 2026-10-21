import React from 'react';

interface NavbarProps {
  title: string;
}

export const Navbar: React.FC<NavbarProps> = ({ title }) => {
  return (
    <header className="topbar">
      <h1 className="topbar-title">{title}</h1>
    </header>
  );
};
