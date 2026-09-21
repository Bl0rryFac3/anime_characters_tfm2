@echo off
setlocal
REM Run this INSIDE tfm2_anime_characters_repo, or it will cd there itself
set "REPO=%~dp0tfm2_anime_characters_repo"

if not exist "%REPO%\.git" (
  echo No .git found, initializing...
  cd /d "%REPO%"
  git init -b main
  git remote add origin https://github.com/Bl0rryFac3/anime_characters_tfm2.git
  echo.
  echo Fetching existing GitHub history...
  git fetch origin
  echo.
  echo If remote has history, merging...
  git pull origin main --allow-unrelated-histories --no-edit
) else (
  cd /d "%REPO%"
)

echo.
echo Current folder: %CD%
echo.

git status
echo.
echo Adding all...
git add -A

echo.
echo Committing...
git commit -m "v0.2.0: fix Saitama tower one-shot, add Kirito - single vfx sheet 1760x1582"

echo.
echo Pushing...
git push -u origin main

echo.
echo Done. If push failed with "fetch first", run:
echo   git pull --rebase origin main
echo   git push
echo.
pause
