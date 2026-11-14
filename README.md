<div align="center">

# ⭕ Kółko i krzyżyk · C#

**Klasyczna gra 3 × 3 w terminalu — dwóch graczy lub pojedynek z komputerem.**

![C#](https://img.shields.io/badge/C%23-.NET%206-512BD4?style=flat-square&logo=dotnet&logoColor=white)
![Platform](https://img.shields.io/badge/Platforma-Konsola-334155?style=flat-square)
![License](https://img.shields.io/badge/Licencja-Apache%202.0-16A34A?style=flat-square)

[🎮 Rozgrywka](#-rozgrywka) · [🚀 Uruchomienie](#-uruchomienie) · [📁 Kod](#-struktura-projektu)

</div>

---

## ✨ W skrócie

| 🎯 Tryb | 🧠 Mechanika | 💾 Historia |
| :--- | :--- | :--- |
| Dwóch graczy lub gracz kontra komputer | Plansza 3 × 3, wykrywanie wygranej i remisu | Wyniki zapisywane w pliku `results.txt` |

## 🎮 Rozgrywka

1. Z menu wybierz **grę dwuosobową** lub **grę z komputerem**.
2. Podaj imię gracza (lub obojga graczy).
3. Wybieraj pola, podając numer wiersza i kolumny.
4. Po zakończeniu partii możesz wyświetlić wcześniejsze wyniki z poziomu menu.

> **AI jest celowo proste:** komputer wybiera losowe wolne pole — nie wykorzystuje algorytmu minimax.

## 🚀 Uruchomienie

Wymagany jest **.NET SDK 6.0** lub środowisko pozwalające zbudować projekt `net6.0`.

~~~bash
git clone https://github.com/wolfgangxxiii/Program.git
cd Program
dotnet run --project Program/Program.csproj
~~~

Aby tylko zbudować aplikację:

~~~bash
dotnet build Program.sln
~~~

> Wyniki są zapisywane do `results.txt` w bieżącym katalogu roboczym procesu.

## 📁 Struktura projektu

| Plik | Odpowiedzialność |
| :--- | :--- |
| [`Program/Program.cs`](Program/Program.cs) | Punkt wejścia |
| [`Program/Controller.cs`](Program/Controller.cs) | Menu, przebieg gry i zapisywanie wyników |
| [`Program/Board.cs`](Program/Board.cs) | Plansza i sprawdzanie wygranej |
| [`Program/HumanPlayer.cs`](Program/HumanPlayer.cs) | Ruchy gracza |
| [`Program/AIPlayer.cs`](Program/AIPlayer.cs) | Ruchy komputera |
| [`Program/Cell.cs`](Program/Cell.cs) i [`Program/GameEntity.cs`](Program/GameEntity.cs) | Model gry |

## 📄 Licencja

[Apache License 2.0](LICENSE.txt) · Projekt: [@wolfgangxxiii](https://github.com/wolfgangxxiii).
